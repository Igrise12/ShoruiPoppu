import { test } from "node:test";
import assert from "node:assert/strict";
import { pdfPage, slideTarget } from "../src/navigation.ts";

test("slide keyboard navigation stays within the presentation", () => {
  assert.equal(slideTarget("ArrowRight", 0, 3), 1);
  assert.equal(slideTarget("PageDown", 2, 3), 2);
  assert.equal(slideTarget("ArrowLeft", 0, 3), 0);
  assert.equal(slideTarget("PageUp", 2, 3), 1);
  assert.equal(slideTarget("Home", 2, 3), 0);
  assert.equal(slideTarget("End", 0, 3), 2);
  assert.equal(slideTarget("x", 0, 3), null);
  assert.equal(slideTarget("End", 0, 0), null);
});

test("PDF page entry accepts whole pages and stays in the document", () => {
  assert.equal(pdfPage(" 4 ", 8), 4);
  assert.equal(pdfPage("0", 8), 1);
  assert.equal(pdfPage("99", 8), 8);
  assert.equal(pdfPage("1.5", 8), null);
  assert.equal(pdfPage("", 8), null);
  assert.equal(pdfPage("99999999999999999999", 8), null);
  assert.equal(pdfPage("1", 0), null);
});
