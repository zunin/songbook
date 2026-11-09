// Original synthetic inputs -> public wasm Session -> core-rendered TeX.
// Used both to regenerate fixtures and by the default smoke to reject stale
// TeX. No notation rendering or book LaTeX is implemented in JavaScript.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

export async function tabFixtures(distDir, fixturesDir) {
  const wasm = await import(pathToFileURL(join(resolve(distDir), "songbook_wasm.js")));
  await wasm.default({ module_or_path: readFileSync(join(distDir, "songbook_wasm_bg.wasm")) });
  const input = (name) => readFileSync(join(fixturesDir, `${name}.txt`), "utf8");
  const mainRaw = input("synthetic-tabs");
  const main = new wasm.SongbookSession(mainRaw);
  const originalRows = mainRaw.split("\n").filter((line) => line.includes("|--"));
  assert.equal(originalRows.length, 18, "three original six-string blocks");
  assert.equal(Math.max(...originalRows.map((line) => line.length)), 56, "exercise the print limit");
  assert.deepEqual(main.previewRows().filter((row) => row.style === "tablature").map((row) => row.text), originalRows);
  assert.deepEqual(main.snapshot().warnings, []);
  const standalone = main.exportTex(true); // unresolved defaults retain tabs
  main.handleAction("confirm");
  main.handleAction("auto_accept");
  assert.equal(main.exportTex(true), standalone, "bulk Keep must not alter tabs");
  const mainPart = main.bookParts();
  assert.equal(mainPart.body, main.exportTex(false));

  const accompaniment = new wasm.SongbookSession(input("synthetic-accompaniment"));
  assert.equal(accompaniment.snapshot().hasTablature, false);

  const partial = new wasm.SongbookSession(input("synthetic-partial-tabs"));
  partial.handleAction("confirm");
  const discarded = "  G|--23--  33--|  ";
  const target = partial.snapshot().lines.find((line) => line.text === discarded).index;
  while (partial.snapshot().position < target) partial.handleAction("next");
  assert.equal(partial.snapshot().position, target);
  partial.handleAction("discard");
  assert(partial.snapshot().warnings.some((text) => text.includes("5 retained rows")));
  partial.handleAction("auto_accept");
  partial.transpose(2);
  assert(partial.snapshot().warnings.some((text) => text.includes("ASCII tabs are unchanged")));
  const partialRows = input("synthetic-partial-tabs").split("\n")
    .filter((line) => line.includes("|--") && line !== discarded);
  assert.deepEqual(partial.previewRows().filter((row) => row.style === "tablature").map((row) => row.text), partialRows);

  const book = wasm.composeBook("Synthetic Tab Songbook", "Songbook Project", [
    accompaniment.bookParts(), mainPart, partial.bookParts(),
  ], false);
  const outputs = new Map([["synthetic-tabs.tex", standalone], ["synthetic-tab-book.tex", book]]);
  main.free();
  accompaniment.free();
  partial.free();
  return { outputs, originalRows, partialRows, discarded };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [distDir, fixturesDir, mode = "--check"] = process.argv.slice(2);
  assert(distDir && fixturesDir && ["--write", "--check"].includes(mode),
    "Usage: node tab-fixtures.mjs <dist-dir> <fixtures-dir> [--write|--check]");
  const { outputs } = await tabFixtures(distDir, fixturesDir);
  for (const [name, tex] of outputs) {
    if (mode === "--write") writeFileSync(join(fixturesDir, name), tex);
    else assert.equal(readFileSync(join(fixturesDir, name), "utf8"), tex, `${name} must be freshly core-generated`);
    console.log(`tab fixtures: ${name} ${mode} — PASS (${tex.length} bytes)`);
  }
}
