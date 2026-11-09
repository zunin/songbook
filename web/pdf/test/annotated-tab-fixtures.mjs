// Original synthetic annotations -> actual public wasm Session -> core TeX.
// Expected literal fragments below test the core plan; no JS typesetting.
import assert from "node:assert/strict";
import { readFileSync, writeFileSync } from "node:fs";
import { resolve, join } from "node:path";
import { pathToFileURL } from "node:url";

export async function annotatedTabFixtures(distDir, fixturesDir) {
  const wasm = await import(pathToFileURL(join(resolve(distDir), "songbook_wasm.js")));
  await wasm.default({ module_or_path: readFileSync(join(distDir, "songbook_wasm_bg.wasm")) });
  const raw = readFileSync(join(fixturesDir, "synthetic-annotated-tabs.txt"), "utf8");
  const source = raw.split("\n");
  const comment = "  Play softly, then let the final notes ring  ";
  const cue = " (C)         (Csus2)       (G)  ";
  const heading = "  C    or   Csus2          G  ";
  const strings = source.filter((line) => line.includes("|--"));
  assert.equal(strings.length, 6, "annotations are not string rows");
  assert.equal(source.indexOf(heading), source.indexOf(cue) + 1, "cue -> heading with 'or' -> strings");
  assert.equal(source.indexOf(strings[0]), source.indexOf(heading) + 1);
  const session = new wasm.SongbookSession(raw);
  for (const name of ["planTabSplit", "applyTabSplit", "cancelTabSplit"]) assert.equal(typeof session[name], "function", name);
  assert.deepEqual(session.snapshot().lines.filter((line) => line.tabRole === "annotation").map((line) => line.text), [cue, heading]);
  assert.equal(session.snapshot().lines.find((line) => line.text === comment).tabRole, undefined);
  session.handleAction("confirm");
  const target = session.snapshot().lines.find((line) => line.text === comment).index;
  while (session.snapshot().position !== target) {
    session.handleAction(session.snapshot().position < target ? "next" : "prev");
  }
  const beforeAttach = session.snapshot().lines;
  session.handleAction("tab_annotation");
  const annotations = [comment, cue, heading];
  const original = session.snapshot();
  const associated = original.lines.filter((line) => line.tabRole);
  assert.deepEqual(associated.map((line) => line.text), [...annotations, ...strings]);
  assert.deepEqual(associated.map((line) => line.tabRole), [...Array(3).fill("annotation"), ...Array(6).fill("string")]);
  assert.equal(associated[0].effectiveLabel, "tab annotation");
  assert.deepEqual(original.lines.filter((line) => line.index !== target), beforeAttach.filter((line) => line.index !== target), "manual attach changes only the selected comment");
  for (const line of associated) {
    assert.equal(line.sourceLine, source.indexOf(line.text) + 1);
    assert.equal(line.tabOrigin, associated[0].tabOrigin);
    assert.equal(line.tabInstance, associated[0].tabInstance);
    for (const field of ["sourceLine", "tabRole", "tabOrigin", "tabInstance"]) assert(Object.hasOwn(line, field), field);
  }
  assert.deepEqual(original.warnings, [], "nine literal rows still mean six strings, not a partial/unsupported group");
  const literal = () => session.previewRows().filter((row) => ["tab-annotation", "tablature"].includes(row.style));
  assert.deepEqual(literal().map((row) => row.text), [...annotations, ...strings]);
  assert.deepEqual(literal().map((row) => row.style), [...Array(3).fill("tab-annotation"), ...Array(6).fill("tablature")]);
  const initialTex = session.exportTex(true);
  assert.equal(session.bookParts().body, session.exportTex(false));
  assert.deepEqual(session.bookParts().chords, ["D", "A"], "annotation chords stay out of the ordinary chord catalogue");

  const afterColumn = 22;
  const plan = session.planTabSplit(afterColumn);
  assert.deepEqual(session.snapshot(), original, "plan is read-only");
  assert.equal(plan.bounds.annotationCount, 3);
  assert.equal(plan.bounds.end - plan.bounds.start, 9);
  assert.equal(plan.annotations.length, 3);
  assert.equal(plan.rows.length, 6);
  assert.equal(plan.afterColumn, afterColumn);
  assert(plan.warnings.some((warning) => warning.includes("annotations appear above both")));
  assert(plan.warnings.some((warning) => warning.includes("no musical alignment is guaranteed")));
  for (const [index, row] of plan.annotations.entries()) {
    assert.equal(row.left, annotations[index].slice(0, afterColumn));
    assert.equal(row.right, " ".repeat(plan.bounds.prefixColumns) + annotations[index].slice(afterColumn));
    assert.equal(row.left + row.right.slice(row.paddingColumns), annotations[index], "annotation characters appear once, in order, without repeated text");
    assert.equal(row.prefix, "");
    assert.equal(row.paddingColumns, plan.bounds.prefixColumns);
    assert.equal(row.sourceLine, source.indexOf(annotations[index]) + 1);
    assert(!row.right.includes(annotations[index].trim()), "right annotation must not repeat the whole comment/cue/heading");
  }
  for (const [index, row] of plan.rows.entries()) {
    assert.equal(row.left, strings[index].slice(0, afterColumn));
    assert.equal(row.right, row.prefix + " ".repeat(row.paddingColumns) + strings[index].slice(afterColumn));
    assert.equal(row.left + row.right.slice(row.prefix.length + row.paddingColumns), strings[index]);
    assert.equal(row.rightLabelOnly, false);
  }
  assert.deepEqual(session.cancelTabSplit(), original, "cancel changes neither review nor undo");
  assert.throws(() => session.applyTabSplit(), "cancelled plan is unusable");
  session.planTabSplit(afterColumn);
  const applied = session.applyTabSplit();
  assert.throws(() => session.applyTabSplit(), "apply is one-use");
  const left = [...plan.annotations, ...plan.rows].map((row) => row.left);
  const right = [...plan.annotations, ...plan.rows].map((row) => row.right);
  const children = applied.lines.filter((line) => line.tabRole);
  assert.deepEqual(children.map((line) => line.text), [...left, ...right], "annotations then six left strings, annotations then six right strings");
  assert.equal(children.filter((line) => line.tabRole === "string").length, 12);
  assert.equal(children.filter((line) => line.tabRole === "annotation").length, 6);
  assert.deepEqual(applied.warnings, []);
  for (const [index, child] of children.entries()) {
    assert.equal(child.sourceLine, associated[index % 9].sourceLine);
    assert.equal(child.tabRole, associated[index % 9].tabRole);
    assert.equal(child.tabOrigin, associated[0].tabOrigin);
    assert.equal(child.tabInstance, children[index < 9 ? 0 : 9].tabInstance);
    assert.notEqual(child.tabInstance, associated[0].tabInstance);
  }
  assert.notEqual(children[0].tabInstance, children[9].tabInstance);
  assert.deepEqual(literal().map((row) => row.text), [...left, ...right]);
  const splitTex = session.exportTex(true);
  const splitPart = session.bookParts();
  assert.equal(splitPart.body, session.exportTex(false));
  assert.deepEqual(splitPart.chords, ["D", "A"]);
  session.handleAction("undo");
  assert.deepEqual(session.snapshot().lines, original.lines, "one undo restores all annotated rows and provenance");
  assert.equal(session.snapshot().position, original.position);
  assert.equal(session.exportTex(true), initialTex);
  session.planTabSplit(afterColumn);
  session.applyTabSplit();
  session.handleAction("auto_accept");
  assert.equal(session.exportTex(true), splitTex, "bulk Keep preserves generated annotation fragments");

  const accompaniment = new wasm.SongbookSession(readFileSync(join(fixturesDir, "synthetic-accompaniment.txt"), "utf8"));
  const bookTex = wasm.composeBook("Synthetic Annotated Songbook", "Songbook Project", [accompaniment.bookParts(), splitPart], false);
  const outputs = new Map([
    ["synthetic-annotated-tabs.tex", initialTex],
    ["synthetic-annotated-tabs-split.tex", splitTex],
    ["synthetic-annotated-tab-book.tex", bookTex],
  ]);
  const cases = [
    { label: "synthetic-annotated-tabs", tex: initialTex, groups: [[...annotations, ...strings]], strings: 6, annotations: 3, book: false, split: false, pages: 1 },
    { label: "synthetic-annotated-tabs-split", tex: splitTex, groups: [left, right], strings: 12, annotations: 6, book: false, split: true, pages: 1 },
    { label: "synthetic-annotated-tab-book", tex: bookTex, groups: [left, right], strings: 12, annotations: 6, book: true, split: true, pages: 4 },
  ];
  session.free();
  accompaniment.free();
  return { outputs, cases };
}

if (process.argv[1] && import.meta.url === pathToFileURL(resolve(process.argv[1])).href) {
  const [distDir, fixturesDir, mode = "--check"] = process.argv.slice(2);
  assert(distDir && fixturesDir && ["--write", "--check"].includes(mode),
    "Usage: node annotated-tab-fixtures.mjs <dist-dir> <fixtures-dir> [--write|--check]");
  const { outputs } = await annotatedTabFixtures(distDir, fixturesDir);
  for (const [name, tex] of outputs) {
    if (mode === "--write") writeFileSync(join(fixturesDir, name), tex);
    else assert.equal(readFileSync(join(fixturesDir, name), "utf8"), tex, `${name} must be freshly core-generated`);
    console.log(`annotated tab fixtures: ${name} ${mode} — PASS (${tex.length} bytes)`);
  }
}
