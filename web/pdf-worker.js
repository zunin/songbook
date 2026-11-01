/**
 * pdf-worker.js — dedicated worker for the browser-local PDF engine.
 *
 * All assets (wasm, TeX bundle, songbook TeX pack) are fetched same-origin
 * from ./engine/ and ./pack/; the engine itself never fetches and no song
 * content is ever posted or fetched over the network. TeX text arrives via
 * postMessage only.
 *
 * Book index pipeline (ADR0019): the songs package's three-step index
 * pipeline cannot complete inside one engine compile — LaTeX pass 1 writes
 * `<index>.sxd` data files, the external songidx step converts one into
 * `<index>.sbx`, and only a later LaTeX pass typesets the real index
 * entries. After each compile the worker inspects the outputs for `*.sxd`
 * files; if any exist it lazily loads the songbook wasm (the
 * `songbook-core::songidx` port of the pinned `songidx.lua`, served from the
 * dist root one level above this worker), converts every `.sxd`, feeds the
 * resulting `.sbx` back with addFile, and recompiles. The loop repeats while
 * a compile still produces `.sxd` content that converts to something new
 * (capped at MAX_COMPILES engine compiles; the pinned demo-book fixture
 * stabilises after 2: the index data does not depend on the typeset pages,
 * so the pass-2 conversion already matches pass 1's). The engine carries its
 * outputs across compiles by design — that is what feeds `.sbx` into the
 * next pass — so `.sxd` files are tracked by content across compiles: a
 * compile that reports a `.sxd` byte-identical to one already converted (a
 * single song compiled after a book, say) needs no new work and no extra
 * pass. Engine intermediates are deliberately kept between compiles —
 * clear_outputs() would force a cold build and discard exactly the state the
 * pipeline feeds back. A single-song compile in a fresh worker produces no
 * `.sxd` and never loads the songbook wasm.
 *
 * Message protocol (main -> worker):
 *   { type: "init" }                       load engine + assets
 *   { type: "compile", id, tex, jobname }  compile TeX text
 *   { type: "dispose" }                    release engine
 * Worker -> main:
 *   { type: "ready", timings }
 *   { type: "result", id, status, bytes?, diagnostics, missingFiles, timings }
 *   { type: "poisoned", id, message }      engine unusable; host must recreate
 *   { type: "error", message }
 */
"use strict";

const ASSET_BASE = new URL(".", self.location);
const LOG = (...a) => console.log("[pdf-worker]", ...a);

// Cap on engine compiles for one compile request. Two are enough for the
// songs package's index pipeline (pass 1 writes the .sxd, pass 2 typesets
// the .sbx converted from it); the slack absorbs documents whose index data
// shifts with the typeset output.
const MAX_COMPILES = 4;

let engine = null;
let readyTimings = null;
let initPromise = null;
let pendingCompile = null;
let songbookWasmPromise = null;
// Index-pipeline memory, scoped to the engine instance (reset on every
// init): which `.sxd` bytes have already been converted, and which `.sbx`
// bytes have already been added to the engine's filesystem.
let pipelineState = null;

async function fetchBuffer(url) {
  const t0 = performance.now();
  const res = await fetch(url);
  if (!res.ok) throw new Error(`fetch ${url}: ${res.status}`);
  const buf = await res.arrayBuffer();
  return { buf, ms: performance.now() - t0 };
}

/** Minimal tar reader for ustar/GNU archives (name@offset, size from header). */
function parseTar(data) {
  const view = new DataView(data);
  const files = new Map();
  let off = 0;
  while (off + 512 <= data.byteLength) {
    const nameBytes = new Uint8Array(data, off, 100);
    const nameEnd = nameBytes.indexOf(0);
    if (nameEnd <= 0) break;
    let name = new TextDecoder().decode(nameBytes.subarray(0, nameEnd));
    const prefixBytes = new Uint8Array(data, off + 345, 155);
    const prefixEnd = prefixBytes.indexOf(0);
    if (prefixEnd > 0) name = new TextDecoder().decode(prefixBytes.subarray(0, prefixEnd)) + "/" + name;
    const sizeStr = new TextDecoder().decode(new Uint8Array(data, off + 124, 12)).replace(/\0.*$/, "").trim();
    const size = parseInt(sizeStr, 8) || 0;
    const type = String.fromCharCode(new Uint8Array(data, off + 156, 1)[0]);
    if (type === "0" || type === "\0") files.set(name, new Uint8Array(data, off + 512, size));
    off += 512 + Math.ceil(size / 512) * 512;
  }
  return files;
}

async function loadGzipTar(url, label) {
  const t0 = performance.now();
  const res = await fetch(url);
  if (!res.ok) throw new Error(`fetch ${url}: ${res.status}`);
  const gz = await res.arrayBuffer();
  const t1 = performance.now();
  const stream = new Response(gz).body.pipeThrough(new DecompressionStream("gzip"));
  const raw = await new Response(stream).arrayBuffer();
  const t2 = performance.now();
  const files = parseTar(raw);
  LOG(`${label}: ${gz.byteLength} B gz -> ${raw.byteLength} B raw, ${files.size} files (fetch ${Math.round(t1 - t0)} ms, decompress+tar ${Math.round(t2 - t1)} ms)`);
  return files;
}

async function init() {
  const t0 = performance.now();
  const { TexEngine } = await import(new URL("engine/index.js", ASSET_BASE).href);
  LOG("engine module imported");
  const wasmResp = await fetch(new URL("engine/tectonic.wasm", ASSET_BASE).href);
  if (!wasmResp.ok) throw new Error(`wasm fetch: ${wasmResp.status}`);
  const tWasmFetch = performance.now();
  engine = await TexEngine.load(wasmResp);
  pipelineState = { sxdSeen: new Map(), sbxFedBack: new Map() };
  const tWasmLoad = performance.now();

  const bundleFiles = await loadGzipTar(new URL("engine/bundle.tar.gz", ASSET_BASE).href, "bundle");
  engine.addFiles(bundleFiles);
  const packFiles = await loadGzipTar(new URL("pack/pack.tar.gz", ASSET_BASE).href, "pack");
  engine.addFiles(packFiles);

  const tAssets = performance.now();
  readyTimings = {
    engineImportMs: null, // not measured separately
    wasmFetchMs: Math.round(tWasmFetch - t0),
    wasmInstantiateMs: Math.round(tWasmLoad - tWasmFetch),
    bundleMs: null,
    packMs: null,
    totalInitMs: Math.round(tAssets - t0),
  };
  return readyTimings;
}

function normalizeMissing(missingFiles, jobname = "main") {
  // Benign-probe whitelist, derived from the recorded missingFiles of a clean
  // compile of the pinned songs fixture against the shipped pack (112 entries,
  // recorded 2026-11-01; see ADR0017). All of them are kpathsea re-probes or
  // the engine's font-format fallback machinery, and all resolve (or are
  // deliberately unused). Every pattern is fully anchored: package requests
  // and any name that carries its own extension still surface — in
  // particular the CM/LM bare-name patterns reject a dot, so "cmr12.tfm"
  // is never mistaken for the benign bare probe "cmr12".
  const job = jobname.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const benign = [
    // songs.sty probes ifpdf.sty (not shipped; songs falls back to its own
    // emulation) and kpathsea re-probes that name with each known extension.
    /^ifpdf\.sty(?:\.(?:aux|bbl|clo|cls|def|fd|ldf|sty|tex))?$/,
    // Multi-pass .aux probes: jobname.aux is (re)probed on every pass, with
    // the same kpathsea extension suffixes. Anchored to the jobname.
    new RegExp(`^${job}\\.aux(?:\\.(?:aux|bbl|clo|cls|def|fd|ldf|sty|tex))?$`),
    // CM fonts are first tried as system/OpenType fonts by bare name before
    // the BaKoMa Type1 pipeline serves them: bare cmr12, cmmi12, ... The
    // bare probe never carries an extension.
    /^cm[a-z]+[0-9]+$/,
    // Latin Modern look-ups (LM is deliberately not shipped; BaKoMa CM is).
    // The font machinery probes each name as: bare name; bare name plus a
    // font container format; a Mac resource fork (name/rsrc); and by raw
    // glyph id (name:0-0-GID, name:0-UCS32-Add).
    /^lm[a-z]+[0-9]+(?:-[a-z]+)?$/,
    /^lm[a-z]+[0-9]+(?:-[a-z]+)?\.(?:TTC|TTF|ttc|ttf|dfont|pfa|pfb)$/,
    /^lm[a-z]+[0-9]+(?:-[a-z]+)?\/rsrc(?:\.(?:TTC|TTF|ttc|ttf|dfont))?$/,
    /^lm[a-z]+[0-9]+(?:-[a-z]+)?:[0-9]+(?:-[A-Za-z0-9]+)*$/,
  ];
  const missing = missingFiles || [];
  const real = missing.filter((n) => !benign.some((re) => re.test(n)));
  return { real, benign: missing.length - real.length };
}

/**
 * Lazily load the songbook wasm glue (the `--target web` module at the dist
 * root, one directory above this worker). The glue's default export fetches
 * `songbook_wasm_bg.wasm` relative to its own URL — also the dist root — so
 * plain relative resolution lands on the deployed artifact. Loaded at most
 * once per worker, and only when a compile actually produces `.sxd` output;
 * single-song compiles never pay for it.
 */
function loadSongbookWasm() {
  if (!songbookWasmPromise) {
    const t0 = performance.now();
    songbookWasmPromise = import(new URL("../songbook_wasm.js", import.meta.url))
      .then(async (songbook) => {
        await songbook.default();
        LOG(`songbook wasm ready in ${Math.round(performance.now() - t0)} ms (index pipeline only)`);
        return songbook;
      });
    // A failed load (e.g. the glue missing from a non-standard deployment)
    // must not poison every later book compile: drop the promise so the next
    // compile retries.
    songbookWasmPromise.catch(() => {
      songbookWasmPromise = null;
    });
  }
  return songbookWasmPromise;
}

const sxdDecoder = new TextDecoder();
const sbxEncoder = new TextEncoder();

/** Converts one `.sxd` byte file into `.sbx` bytes via the songidx port. */
async function convertSxd(name, bytes) {
  const songbook = await loadSongbookWasm();
  const sbx = songbook.convert_sxd_to_sbx(sxdDecoder.decode(bytes));
  return sbxEncoder.encode(sbx);
}

function bytesEqual(a, b) {
  if (a === b) return true;
  if (!a || !b || a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

/**
 * One engine compile request, running the multi-pass book index pipeline
 * when the document produces `.sxd` index data. Returns the final engine
 * result plus the pass log for diagnostics.
 */
async function runCompiles(id) {
  const t0 = performance.now();
  let result = engine.compile({ entry: "main.tex", paper: "a4" });
  const passes = [{ status: result.status, ms: Math.round(performance.now() - t0) }];
  LOG("compile", id, "pass 1 status=", result.status, passes[0].ms, "ms");

  // `.sxd` present → the songs package wants its index pipeline. Convert and
  // feed back until a compile stops producing anything new. The engine
  // carries outputs (and any added `.sbx`) across compiles, so `.sxd` bytes
  // already converted count as satisfied even when a later compile re-reports
  // them.
  const { sxdSeen, sbxFedBack } = pipelineState;
  while (result.outputs.some((o) => o.name.endsWith(".sxd"))) {
    let added = 0;
    for (const { name } of result.outputs.filter((o) => o.name.endsWith(".sxd"))) {
      const bytes = engine.output(name);
      if (!bytes) {
        throw new Error(`engine listed ${name} in its outputs but could not return its bytes`);
      }
      if (bytesEqual(sxdSeen.get(name), bytes)) continue;
      const sbxName = name.slice(0, -".sxd".length) + ".sbx";
      // Convert before recording anything: a conversion error must leave the
      // state untouched so a later compile retries this file.
      const sbxBytes = await convertSxd(name, bytes);
      sxdSeen.set(name, bytes);
      if (bytesEqual(sbxFedBack.get(sbxName), sbxBytes)) continue;
      sbxFedBack.set(sbxName, sbxBytes);
      engine.addFile(sbxName, sbxBytes);
      added++;
      LOG("index pipeline: pass", passes.length, name, bytes.length, "B ->", sbxName, sbxBytes.length, "B (new)");
    }
    if (added === 0) {
      LOG("index pipeline: stable after", passes.length, "engine compiles");
      break;
    }
    if (passes.length >= MAX_COMPILES) {
      LOG("index pipeline: cap of", MAX_COMPILES, "engine compiles reached with", added, "new sbx; returning last result");
      break;
    }
    const t = performance.now();
    result = engine.compile({ entry: "main.tex", paper: "a4" });
    passes.push({ status: result.status, ms: Math.round(performance.now() - t) });
    LOG("compile", id, "pass", passes.length, "status=", result.status, passes[passes.length - 1].ms, "ms");
  }
  return { result, passes };
}

async function compile(msg) {
  const t0 = performance.now();
  engine.addFile("main.tex", msg.tex);
  const { result, passes } = await runCompiles(msg.id);
  const t1 = performance.now();
  const pdf = engine.pdf();
  const { real, benign } = normalizeMissing(result.missingFiles, msg.jobname);
  const timings = {
    compileMs: Math.round(t1 - t0),
    pdfBytes: pdf ? pdf.length : 0,
  };
  LOG(
    "compile", msg.id, "done:", passes.length, "engine pass(es),",
    timings.pdfBytes, "B pdf,", timings.compileMs, "ms total",
    passes.map((p) => p.status).join("/"),
  );
  if (pdf) {
    const bytes = pdf.slice();
    self.postMessage({ type: "result", id: msg.id, status: result.status, bytes, diagnostics: result.diagnostics, missingFiles: real, benignProbes: benign, timings }, [bytes.buffer]);
  } else {
    self.postMessage({ type: "result", id: msg.id, status: result.status, bytes: null, diagnostics: result.diagnostics, missingFiles: real, benignProbes: benign, timings });
  }
}

self.onmessage = async (e) => {
  const msg = e.data;
  try {
    if (msg.type === "init") {
      if (engine) {
        // Idempotent: assets are already loaded, so answer immediately
        // instead of re-fetching the ~19 MB payload.
        self.postMessage({ type: "ready", timings: readyTimings });
        return;
      }
      // Share one in-flight load between concurrent init requests, so a
      // second init while assets are downloading never double-fetches.
      if (!initPromise) {
        initPromise = init()
          .catch((err) => {
            // Never leave a half-loaded engine behind: it would wrongly
            // satisfy the idempotency guard above on the next init.
            engine = null;
            readyTimings = null;
            pipelineState = null;
            throw err;
          })
          .finally(() => {
            initPromise = null;
          });
      }
      await initPromise;
      self.postMessage({ type: "ready", timings: readyTimings });
    } else if (msg.type === "compile") {
      if (!engine) throw new Error("worker not initialised");
      await compile(msg);
    } else if (msg.type === "dispose") {
      engine = null;
      pipelineState = null;
      self.postMessage({ type: "disposed" });
    } else {
      self.postMessage({ type: "error", message: `unknown message type ${msg?.type}` });
    }
  } catch (err) {
    LOG("worker error:", err && err.message);
    if (err && (err.name === "EnginePoisonedError" || /EnginePoisoned|poison/i.test(String(err.message)))) {
      self.postMessage({ type: "poisoned", id: msg?.id, message: String(err.message) });
    } else {
      self.postMessage({ type: "error", id: msg?.id, message: String(err && err.message || err) });
    }
  }
};

LOG("pdf-worker module loaded");
