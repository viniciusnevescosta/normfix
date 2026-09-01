import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "vitest";

const responsive = await readFile(new URL("../src/styles/responsive.css", import.meta.url), "utf8");

function section(start: string, end?: string): string {
  const from = responsive.indexOf(start);
  assert.notEqual(from, -1, `${start} exists`);
  const to = end ? responsive.indexOf(end, from + start.length) : responsive.length;
  assert.notEqual(to, -1, `${end} exists after ${start}`);
  return responsive.slice(from, to);
}

test("mobile layouts stack without imposing a viewport-wide minimum", () => {
  const mobile = section("@media (max-width: 900px)", "@media (max-width: 560px)");
  assert.match(mobile, /\.workbench\s*\{[\s\S]*?grid-template-columns:\s*minmax\(0, 1fr\)/);
  assert.doesNotMatch(mobile, /min-width:\s*\d+px/);
});

test("coarse pointers keep primary controls at a comfortable touch size", () => {
  const coarse = section("@media (pointer: coarse)");
  for (const selector of [
    ".button",
    ".icon-button",
    ".text-button",
    ".brand",
    ".release-link",
    "#top-bar select",
    '#result-summary [role="tab"]',
    "#status-badges button",
    "dialog button",
    '[aria-haspopup="menu"]',
    '[role="treeitem"]',
  ]) {
    assert.ok(coarse.includes(selector), `${selector} participates in coarse-pointer sizing`);
  }
  assert.match(coarse, /min-height:\s*44px/);
  assert.doesNotMatch(coarse, /#top-bar a/);
  assert.match(coarse, /\.icon-button,[\s\S]*?\[aria-haspopup="menu"\][\s\S]*?width:\s*44px/);
});

test("documentation and GitHub stay compact on phones", () => {
  const phone = section("@media (max-width: 560px)", "/* Phones in landscape");
  assert.match(phone, /#top-bar \.topbar-link\s*\{[\s\S]*?min-height:\s*30px/);
});

test("narrow landscape preserves a two-pane working layout", () => {
  const landscape = section(
    "@media (min-width: 620px) and (max-width: 900px) and (max-height: 520px)",
    "@media (pointer: coarse)",
  );
  assert.match(
    landscape,
    /\.workbench\s*\{[\s\S]*?grid-template-columns:\s*minmax\(190px, 31vw\) minmax\(0, 1fr\)/,
  );
});
