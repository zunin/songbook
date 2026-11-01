/**
 * pdf-engine.js — browser-local PDF engine API for the songbook app.
 *
 * Accepts generated LaTeX text and returns PDF bytes via a dedicated worker.
 * All assets (wasm, TeX bundle, songbook TeX pack, fonts) are served as
 * static files from `pdf-assets/` at the app's own origin; the engine never
 * fetches and no song content is ever posted or fetched over the network.
 *
 * The caller can cancel/terminate/recreate the engine after a poisoned
 * compile. Cancellation kills the worker immediately; the next compile
 * transparently respawns a fresh worker and reloads assets.
 *
 * Usage:
 *   const engine = await createPdfEngine();          // worker starts lazily
 *   await engine.ready();                             // preload assets
 *   const { blob, bytes, status, ... } = await engine.compile(tex);
 *   await engine.cancel();                            // abort + respawn next use
 *   await engine.recreate();                          // fresh worker now
 *   await engine.dispose();                           // final cleanup
 *
 * Exports createPdfEngine (named) for ESM consumers.
 */
"use strict";

const DEFAULT_ASSET_BASE = "pdf-assets";

// Under `require-trusted-types-for 'script'` the Worker constructor's URL is a
// TrustedScriptURL sink, so a plain string throws before the worker can start.
// We mint exactly one narrowly-scoped policy (allowlisted by name in the CSP)
// whose only job is the worker URL. Same-origin enforcement here is a second
// line of defence: even if a caller passes an absolute/assetBase override, the
// worker can never be pointed at another origin (which a worker script would
// otherwise be able to do).
let workerUrlPolicy = null;

function workerScriptUrl(url) {
  const tt = globalThis.trustedTypes;
  // Firefox/Safari have no Trusted Types; a plain string is the correct input.
  if (!tt || typeof tt.createPolicy !== "function") return url;
  if (!workerUrlPolicy) {
    const createScriptURL = (value) => {
      const resolved = new URL(value, document.baseURI);
      if (resolved.origin !== location.origin) {
        throw new TypeError(`pdf-engine: refusing cross-origin worker URL ${resolved.href}`);
      }
      return resolved.href;
    };
    try {
      workerUrlPolicy = tt.createPolicy("pdf-engine", { createScriptURL });
    } catch (err) {
      // Another copy of this module (or a later reload) may have already
      // created the policy; the browser only allows one policy per name.
      if (typeof tt.getPolicy === "function") {
        workerUrlPolicy = tt.getPolicy("pdf-engine");
      }
      if (!workerUrlPolicy) throw err;
    }
  }
  return workerUrlPolicy.createScriptURL(url);
}

export async function createPdfEngine({ assetBase = DEFAULT_ASSET_BASE, onProgress } = {}) {
  const log = (...a) => console.log("[pdf-engine]", ...a);
  const base = assetBase.replace(/\/+$/, "");

  let worker = null;
  let nextId = 0;
  let disposed = false;
  const pendingInits = new Set(); // reject hooks for ready() calls awaiting init

  function spawnWorker() {
    if (disposed) throw new Error("pdf engine disposed");
    worker = new Worker(workerScriptUrl(`${base}/pdf-worker.js`), { type: "module" });
    return worker;
  }

  function terminateWorker() {
    if (worker) {
      worker.terminate();
      worker = null;
      log("worker terminated");
    }
    // A pending init cannot finish without its worker; reject those waiters
    // so a caller awaiting ready() (e.g. compile()) never hangs forever.
    for (const fail of pendingInits) {
      fail(new DOMException("PDF engine worker terminated", "AbortError"));
    }
    pendingInits.clear();
  }

  /** Per-caller abort handling; the shared init itself keeps running. */
  function withAbort(promise, signal) {
    if (!signal) return promise;
    if (signal.aborted) {
      return Promise.reject(new DOMException("Aborted", "AbortError"));
    }
    return new Promise((resolve, reject) => {
      const onAbort = () => reject(new DOMException("Aborted", "AbortError"));
      signal.addEventListener("abort", onAbort, { once: true });
      promise.then(
        (value) => {
          signal.removeEventListener("abort", onAbort);
          resolve(value);
        },
        (error) => {
          signal.removeEventListener("abort", onAbort);
          reject(error);
        },
      );
    });
  }

  function ready({ signal } = {}) {
    // Worker is (re)created and initialised on demand. Returns once "ready".
    // The worker's init is idempotent, so repeat calls are cheap no-ops.
    if (signal && signal.aborted) {
      return Promise.reject(new DOMException("Aborted", "AbortError"));
    }
    if (disposed) {
      return Promise.reject(new Error("pdf engine disposed"));
    }
    const w = worker ?? spawnWorker();
    const init = new Promise((resolve, reject) => {
      let settled = false;
      const cleanup = () => {
        w.removeEventListener("message", onMsg);
        w.removeEventListener("error", onError);
        pendingInits.delete(fail);
      };
      const settle = (fn, value) => {
        if (settled) return;
        settled = true;
        cleanup();
        fn(value);
      };
      const onMsg = (e) => {
        const m = e.data;
        if (m.type === "ready") settle(resolve, m.timings);
        else if (m.type === "error") settle(reject, new Error(m.message));
      };
      const onError = (ev) => {
        settle(reject, ev.error || new Error("worker error"));
      };
      const fail = (error) => settle(reject, error);
      pendingInits.add(fail);
      w.addEventListener("message", onMsg);
      w.addEventListener("error", onError);
      w.postMessage({ type: "init" });
    });
    return withAbort(init, signal);
  }

  async function compile(tex, { jobname = "main", signal } = {}) {
    if (typeof tex !== "string" || !tex.trim()) {
      throw new Error("compile() requires non-empty TeX text");
    }
    if (signal && signal.aborted) throw new DOMException("Aborted", "AbortError");
    // A fresh worker is not initialised yet: wait for its assets before
    // posting the job, otherwise the worker rejects with "not initialised".
    await ready({ signal });
    if (signal && signal.aborted) throw new DOMException("Aborted", "AbortError");
    const w = worker;
    if (!w) throw new DOMException("PDF engine worker terminated", "AbortError");
    const id = ++nextId;
    return new Promise((resolve, reject) => {
      let settled = false;
      const onMsg = (e) => {
        const m = e.data;
        if (m.id !== id) return;
        if (m.type === "result") {
          cleanup();
          settled = true;
          resolve({
            id,
            status: m.status,
            bytes: m.bytes ? new Uint8Array(m.bytes) : null,
            blob: m.bytes ? new Blob([m.bytes], { type: "application/pdf" }) : null,
            diagnostics: m.diagnostics,
            missingFiles: m.missingFiles,
            benignProbes: m.benignProbes,
            timings: m.timings,
          });
        } else if (m.type === "poisoned") {
          cleanup();
          // Engine is unusable: recreate immediately so the next compile works.
          log("poisoned compile — recreating worker");
          recreate().catch(() => {});
          reject(new Error(`poisoned compile: ${m.message}`));
        } else if (m.type === "error") {
          cleanup();
          reject(new Error(m.message));
        }
      };
      const onAbort = () => {
        cleanup();
        terminateWorker(); // hard cancel — next compile respawns
        reject(new DOMException("Aborted", "AbortError"));
      };
      const onError = (ev) => { cleanup(); reject(ev.error || new Error("worker error")); };
      const cleanup = () => {
        w.removeEventListener("message", onMsg);
        w.removeEventListener("error", onError);
        if (signal) signal.removeEventListener("abort", onAbort);
      };
      w.addEventListener("message", onMsg);
      w.addEventListener("error", onError);
      if (signal) signal.addEventListener("abort", onAbort, { once: true });
      w.postMessage({ type: "compile", id, tex, jobname });
      if (onProgress) onProgress({ stage: "compiling", id });
    });
  }

  async function cancel() {
    terminateWorker();
  }

  async function recreate() {
    terminateWorker();
    await ready();
  }

  async function dispose() {
    disposed = true;
    terminateWorker();
  }

  return { compile, cancel, ready, recreate, dispose, isReady: () => worker !== null };
}

export default createPdfEngine;
