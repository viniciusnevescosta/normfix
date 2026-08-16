// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import ResultSummary from "../src/components/ResultSummary.svelte";

function open(overrides: Record<string, unknown> = {}) {
  const calls: Array<[string, ...unknown[]]> = [];
  const { container } = render(ResultSummary, {
    props: {
      summary: { files: 2, changed: 1, fixes: 13, diagnostics: 1, failed: 0 },
      paths: ["main.c", "src/utils.c"],
      selected: "main.c",
      usable: true,
      applicable: 1,
      diagnosticCount: 1,
      view: "formatted",
      copyLabel: "copyFile",
      onSelect: (path: string) => calls.push(["select", path]),
      onView: (view: string) => calls.push(["view", view]),
      onApply: () => calls.push(["apply"]),
      onApplyAll: () => calls.push(["applyAll"]),
      onCopy: () => calls.push(["copy"]),
      onDownload: () => calls.push(["download"]),
      onDownloadAll: () => calls.push(["downloadAll"]),
      ...overrides,
    },
  });
  const button = (label: string): HTMLButtonElement | undefined =>
    [...container.querySelectorAll("button")].find((item) =>
      item.textContent?.trim().startsWith(label),
    );
  return { calls, container, button };
}

test("the counts are shown with the words that name them", () => {
  const panel = open();
  const text = panel.container.textContent ?? "";

  // The words come from the catalogue now rather than from a stub, so this
  // also proves the keys exist.
  for (const label of ["files", "changed", "fixes", "diagnostics", "failed"]) {
    assert.match(text, new RegExp(label));
  }
  assert.match(text, /13/, "the fix count is the number, not a description of it");
});

test("a result that cannot be used leaves no button saying it can", () => {
  const panel = open({ usable: false, applicable: 0 });

  for (const label of ["Fix this file", "copyFile", "Download file", "Fix all files"]) {
    assert.equal(panel.button(label)?.disabled, true, `${label} is disabled`);
  }
  // Downloading the whole project is not about this one file, so it stays.
  assert.equal(panel.button("Download all")?.disabled, false);
});

test("choosing a file and a view reports which was chosen", () => {
  const panel = open();

  const picker = panel.container.querySelector("select");
  assert.ok(picker);
  picker.value = "src/utils.c";
  picker.dispatchEvent(new Event("change", { bubbles: true }));

  panel.button("Diff")?.click();

  assert.deepEqual(panel.calls, [
    ["select", "src/utils.c"],
    ["view", "diff"],
  ]);
});

test("the open view is the one marked selected, and only it", () => {
  const panel = open({ view: "diagnostics" });
  const tabs = [...panel.container.querySelectorAll("[role=tab]")];

  assert.deepEqual(
    tabs.map((tab) => tab.getAttribute("aria-selected")),
    ["false", "true", "false"],
  );
});
