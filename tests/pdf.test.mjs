import { test } from "node:test";
import assert from "node:assert/strict";
import { readFile, access } from "node:fs/promises";
import { resolve } from "node:path";
import { getDocument, OPS } from "pdfjs-dist/legacy/build/pdf.mjs";

test("PDF.js opens a local image-heavy PDF with bundled fonts and page content", async () => {
  const assetRoot = resolve("dist/pdfjs");
  await Promise.all([
    access(`${assetRoot}/standard_fonts/LiberationSans-Regular.ttf`),
    access(`${assetRoot}/cmaps/78-EUC-H.bcmap`),
    access(`${assetRoot}/wasm/openjpeg.wasm`),
  ]);
  const bytes = await readFile("src-tauri/tests/fixtures/example.pdf");
  const task = getDocument({ data: new Uint8Array(bytes), standardFontDataUrl: `${assetRoot}/standard_fonts/` });
  try {
    const pdf = await task.promise;
    assert.equal(pdf.numPages, 2);
    const page = await pdf.getPage(1);
    const text = await page.getTextContent();
    assert.ok(text.items.some(item => "str" in item && item.str.includes("Hello PDF")));
    const operators = await page.getOperatorList();
    assert.ok(operators.fnArray.includes(OPS.paintImageXObject) || operators.fnArray.includes(OPS.paintInlineImageXObject));
  } finally { await task.destroy(); }
});
