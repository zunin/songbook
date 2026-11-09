// Independent checks beyond native text parity: exact core payload tokens,
// glyph order, fixed-cell spaces, and atomic blocks in actual PDF coordinates.
import assert from "node:assert/strict";

export function assertTabTex(tex, expectedRows) {
  assert(tex.includes("\\songcolumns{1}\n"), "tabs require a single song column");
  assert(tex.includes("\\fontfamily{cmtt}\\fontsize{10}{12}\\selectfont"), "readable 10pt cmtt");
  const rows = [...tex.matchAll(/^\\hbox\{(.*)\}$/gm)].map(([, body]) => {
    const tokens = [...body.matchAll(/\\(?:char(\d+)|hskip\\dimen0)\\relax /g)];
    assert.equal(tokens.map(([token]) => token).join(""), body, "only inert numeric glyphs / measured spaces");
    return tokens.map(([, code]) => {
      if (code === undefined) return " ";
      const slot = Number(code);
      return String.fromCharCode(slot === 13 ? 39 : slot === 18 ? 96 : slot);
    }).join("");
  });
  assert.deepEqual(rows, expectedRows, "literal rows, including indentation and trailing spaces, survive core rendering");
  assert(rows.every((row) => row.length <= 56));
}

const decodeXml = (text) => text.replace(/&(amp|lt|gt|quot|apos);/g, (_, name) =>
  ({ amp: "&", lt: "<", gt: ">", quot: '"', apos: "'" })[name]);
const near = (got, want, message) => assert(Math.abs(got - want) < 0.04, `${message}: got ${got}, want ${want}`);

export function assertTabPdf(layoutText, bboxXml, groups) {
  const pages = [...bboxXml.matchAll(/<page width="([\d.]+)" height="([\d.]+)">([\s\S]*?)<\/page>/g)]
    .map(([, width, height, body]) => ({
      width: Number(width), height: Number(height),
      words: [...body.matchAll(/<word xMin="([\d.]+)" yMin="([\d.]+)" xMax="([\d.]+)" yMax="([\d.]+)">([\s\S]*?)<\/word>/g)]
        .map(([, x, y, right, bottom, text]) => ({
          x: Number(x), y: Number(y), right: Number(right), bottom: Number(bottom), text: decodeXml(text),
        })),
    }));
  assert(pages.length, "pdftotext must expose PDF page coordinates");
  const compactLines = layoutText.split("\n").map((line) => line.replace(/\s/g, ""));
  let previousPage = -1;
  let previousY = -1;
  for (const group of groups) {
    let first = null;
    for (const [rowIndex, row] of group.entries()) {
      assert(compactLines.includes(row.replace(/\s/g, "")), `all glyphs in order: ${row}`);
      const chunks = [...row.matchAll(/\S+/g)];
      // Match the complete physical line, not just its first token: annotation
      // fragments can start with a word also present in ordinary lyrics.
      const candidates = pages.flatMap((page, pageIndex) => page.words
        .filter((word) => word.text === chunks[0][0])
        .map((start) => ({
          pageIndex, start,
          words: page.words.filter((word) => Math.abs(word.y - start.y) < 0.01).sort((a, b) => a.x - b.x),
        })))
        .filter(({ pageIndex, start, words }) =>
          (pageIndex > previousPage || (pageIndex === previousPage && start.y > previousY)) &&
          words.map((word) => word.text).join("\0") === chunks.map(([text]) => text).join("\0"));
      assert.equal(candidates.length, 1, `one complete PDF row, without loss, repetition or reordering: ${row}`);
      const { pageIndex, start, words } = candidates[0];
      const page = pages[pageIndex];
      if (!first) {
        const cell = (start.right - start.x) / chunks[0][0].length;
        first = { pageIndex, x: start.x - chunks[0].index * cell, y: start.y, cell };
        // TeX points are 72.27/in; PDF points are 72/in. cmtt10 cell is
        // approximately 5.25 TeX pt; 12pt baseline spacing is 11.955 PDF pt.
        near(first.cell, 5.25 * 72 / 72.27, "10pt cmtt cell width");
        assert(pageIndex > previousPage || (pageIndex === previousPage && start.y > previousY), "tab group order");
      }
      assert.equal(pageIndex, first.pageIndex, "a retained tab block cannot split across pages");
      near(start.x, first.x + chunks[0].index * first.cell, "raw-column alignment, including annotation indentation");
      near(start.y, first.y + rowIndex * 12 * 72 / 72.27, "12pt baseline spacing / string order");
      for (const [index, chunk] of chunks.entries()) {
        near(words[index].x, first.x + chunk.index * first.cell, "spaces occupy exact measured cells");
        near(words[index].right - words[index].x, chunk[0].length * first.cell, "fixed-cell glyph width");
        assert(words[index].x >= 0 && words[index].right < page.width && words[index].bottom < page.height, "no glyphs outside the page");
      }
      previousPage = pageIndex;
      previousY = start.y;
    }
  }
  return pages.length;
}

export function assertAnnotatedOrder(layoutText, fixture) {
  const compact = layoutText.replace(/\s/g, "");
  const samples = [
    "Before the annotated six strings",
    ...fixture.groups.flat(),
    "After the annotated six strings",
    "Ordinary chord and lyric material stays later",
    "No annotation is counted as another string",
  ];
  if (fixture.book) {
    samples.unshift("First words lead to the strings", "Pure chord and lyric coverage still remains", "This song has no tablature");
    assert(layoutText.includes("Index of Songs") && !layoutText.includes("Index not yet generated"), "real annotated-book songidx index");
  }
  assert(!layoutText.includes("Partial six-string"), "annotations never count as extra strings");
  let position = -1;
  for (const sample of samples) {
    const found = compact.indexOf(sample.replace(/\s/g, ""), position + 1);
    assert(found > position, `annotation/string/ordinary-lyric order: ${sample}`);
    position = found;
  }
}

export function assertMixedOrder(layoutText, book = false) {
  const text = layoutText.replace(/\s+/g, " ");
  const samples = [
    "Before the first six strings", "e|--0--12h14p12", "E|--5--[7]",
    "After the first six strings", "Before the alternate labels", "D#|--10--12--14--|",
    "Eb|--15--17--19--|", "After the alternate labels", "Before the alternate labels",
    "D#|--20--22--24--|", "Eb|--25--27--29--|", "After the alternate labels",
    "After both distinct choruses",
  ];
  if (book) {
    samples.unshift("First words lead to the strings", "Pure chord and lyric coverage still remains", "This song has no tablature");
    samples.push("Chord transposition is active; ASCII tabs are unchanged and may no longer match.",
      "Before the reviewed strings", "Partial six-string tab block: 5 retained rows; source line 6.",
      "e|--21--", "E|--26--", "After the reviewed strings");
    assert(!text.includes("G|--23--"), "a discarded string must not be invented");
    assert(text.includes("Index of Songs") && !text.includes("Index not yet generated"), "real songidx index");
  }
  let position = -1;
  for (const sample of samples) {
    const found = text.indexOf(sample, position + 1);
    assert(found > position, `meaningful content / mixed ordering: ${sample}`);
    position = found;
  }
}
