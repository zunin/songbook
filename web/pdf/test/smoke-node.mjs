// smoke-node.mjs — offline Node smoke test for the assembled PDF assets.
// Verifies that the docstrip-generated songs.sty / size12.clo and the BaKoMa
// fonts compile the standalone songs fixture in the GlyphTeX engine and
// produce the same PDF text as the native Tectonic reference.
// Usage: node smoke-node.mjs <assets-dir> <fixtures-dir>
import { readFileSync, writeFileSync, readdirSync, statSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { join, basename, resolve } from "node:path";

const assetsDir = resolve(process.argv[2]);
const fixturesDir = process.argv[3];

// 1. load the vendored engine from the built assets
const engineMod = await import(new URL(`file://${assetsDir}/engine/index.js`).href);
const TexEngine = engineMod.TexEngine;

// 2. wasm + bundle
const wasm = new Uint8Array(readFileSync(`${assetsDir}/engine/tectonic.wasm`));
const engine = await TexEngine.load(wasm);

function walk(dir, cb) {
  for (const e of readdirSync(dir)) {
    const p = join(dir, e);
    if (statSync(p).isDirectory()) walk(p, cb);
    else cb(e, p);
  }
}

// 3. bundle.tar.gz -> files (Node has zlib + a tiny tar reader)
const { gzipSync, gunzipSync } = await import("node:zlib");
function parseTar(data) {
  const files = new Map();
  let off = 0;
  while (off + 512 <= data.length) {
    const name = data.subarray(off, off + 100).indexOf(0) > 0
      ? data.subarray(off, off + 100).subarray(0, data.subarray(off, off + 100).indexOf(0)).toString()
      : "";
    const size = parseInt(data.subarray(off + 124, off + 136).toString().replace(/\0.*$/, "").trim(), 8) || 0;
    const type = String.fromCharCode(data[off + 156]);
    if (type === "0" || type === "\0") files.set(name, data.subarray(off + 512, off + 512 + size));
    off += 512 + Math.ceil(size / 512) * 512;
  }
  return files;
}
const bundleFiles = parseTar(gunzipSync(readFileSync(`${assetsDir}/engine/bundle.tar.gz`)));
for (const [name, data] of bundleFiles) engine.addFile(name, data);

// 4. pack.tar.gz (generated songs.sty, size12.clo, fonts)
const packFiles = parseTar(gunzipSync(readFileSync(`${assetsDir}/pack/pack.tar.gz`)));
for (const [name, data] of packFiles) engine.addFile(name, data);
console.log(`smoke: bundle=${bundleFiles.size} pack=${packFiles.size} files loaded`);

// 5. compile the standalone fixture
const tex = readFileSync(`${fixturesDir}/let-it-be.tex`, "utf8");
engine.addFile("main.tex", tex);
const t0 = Date.now();
const result = engine.compile({ entry: "main.tex", paper: "a4" });
const compileMs = Date.now() - t0;
const pdf = engine.pdf();
const missing = result.missingFiles || [];
console.log(`smoke: status=${result.status} pdf=${pdf ? pdf.length : "MISSING"} B compile=${compileMs} ms missing=${missing.length}`);

if (!pdf || result.status !== "spotless") {
  console.error("smoke: FAILED", { status: result.status, missing, diagnostics: result.diagnostics });
  process.exit(1);
}

// 6. compare extracted text against the pinned native reference
writeFileSync("/tmp/pdf-engine-smoke-out.pdf", pdf);
const got = execFileSync("pdftotext", ["/tmp/pdf-engine-smoke-out.pdf", "-"]).toString();
const want = readFileSync(`${fixturesDir}/let-it-be-reference.txt`, "utf8");
if (got.trim() !== want.trim()) {
  console.error("smoke: PDF text differs from native reference");
  process.exit(1);
}
console.log("smoke: PDF text matches native Tectonic reference — PASS");

// ---------------------------------------------------------------------------
// 7. multi-song book fixture (ADR0018 fitness function).
// Known limitation (recorded, Phase 2): a single-pass compile cannot run the
// songs package's 3-step index pipeline (songidx.lua), so index PAGES render
// the package's placeholder instead of sorted entries. The smoke tolerates
// the placeholder but still requires the document to compile and both songs
// to be present in the extracted text.
const bookTex = readFileSync(`${fixturesDir}/demo-book.tex`, "utf8");
engine.addFile("book.tex", bookTex);
const tb0 = Date.now();
const bookResult = engine.compile({ entry: "book.tex", paper: "a4" });
const bookMs = Date.now() - tb0;
const bookPdf = engine.pdf();
const bookMissing = bookResult.missingFiles || [];
console.log(`smoke: book status=${bookResult.status} pdf=${bookPdf ? bookPdf.length : "MISSING"} B compile=${bookMs} ms missing=${bookMissing.length}`);

if (!bookPdf || bookResult.status !== "spotless") {
  console.error("smoke: book FAILED (compile)", {
    status: bookResult.status,
    missing: bookMissing,
    diagnostics: bookResult.diagnostics,
  });
  process.exit(1);
}

writeFileSync("/tmp/pdf-engine-smoke-book.pdf", bookPdf);
const bookText = execFileSync("pdftotext", ["/tmp/pdf-engine-smoke-book.pdf", "-"]).toString();

// pdftotext emits one form feed (\f) after every page, including the last
// (verified empirically: the 1-page let-it-be fixture yields exactly 1 \f at
// end-of-output), so the page count equals the number of \f characters.
const bookPages = (bookText.match(/\f/g) || []).length;

// Cross-check the convention with pdfinfo when it is installed.
let pdfinfoPages = null;
try {
  const info = execFileSync("pdfinfo", ["/tmp/pdf-engine-smoke-book.pdf"]).toString();
  const m = info.match(/^Pages:\s+(\d+)\s*$/m);
  if (m) pdfinfoPages = parseInt(m[1], 10);
} catch (err) {
  if (err.code !== "ENOENT") throw err;
  console.log("smoke: book pdfinfo not installed — skipping page-count cross-check");
}
if (pdfinfoPages !== null) {
  console.log(`smoke: book pages=${bookPages} (pdfinfo: ${pdfinfoPages})`);
  if (pdfinfoPages !== bookPages) {
    console.error(`smoke: book FAILED (page-count cross-check): \\f count=${bookPages} vs pdfinfo=${pdfinfoPages}`);
    process.exit(1);
  }
} else {
  console.log(`smoke: book pages=${bookPages}`);
}

if (bookPages < 3) {
  console.error(`smoke: book FAILED (page count): expected >= 3 (title + index + songs), got ${bookPages}`);
  process.exit(1);
}

const bookChecks = [
  ["title page heading", "Demo Songbook"],
  ["index page heading", "Index of Songs"],
  ["song title 'Let It Be'", "Let It Be"],
  ["song title 'Every Construct'", "Every Construct"],
  ["lyric from 'Let It Be'", "Mother Mary comes to me"],
  ["lyric from 'Every Construct'", "Plain ambiguous lyric line"],
];
const missed = bookChecks.filter(([, needle]) => !bookText.includes(needle));
if (missed.length > 0) {
  console.error("smoke: book FAILED (text checks), missing:", missed.map(([label]) => label));
  process.exit(1);
}

// Tolerated engine limitation (Phase 2): unsorted index pages.
if (bookText.includes("Index not yet generated")) {
  console.log("smoke: book index placeholder present (expected: single-pass compile cannot run songidx pipeline)");
}

console.log(`smoke: book PASS status=spotless pages=${bookPages} songs=2/2 lyrics=2/2 index-heading OK`);
