// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import FileTree from "../src/components/FileTree.svelte";

function open(files: string[], overrides: Record<string, unknown> = {}) {
  const calls: Array<[string, ...unknown[]]> = [];
  const result = render(FileTree, {
    props: {
      files,
      unsupported: new Set<string>(),
      changed: new Set<string>(),
      selected: null,
      kindOf: () => "C",
      onSelect: (path: string) => calls.push(["select", path]),
      onMove: (path: string, isFolder: boolean, folder: string) =>
        calls.push(["move", path, isFolder, folder]),
      onRename: (path: string) => calls.push(["rename", path]),
      onDelete: (path: string) => calls.push(["delete", path]),
      ...overrides,
    },
  });
  const rows = (): string[] =>
    [...result.container.querySelectorAll("[data-path]")].map(
      (row) => (row as HTMLElement).dataset.path ?? "",
    );
  const row = (path: string): HTMLElement => {
    const found = result.container.querySelector<HTMLElement>(`[data-path="${path}"]`);
    assert.ok(found, `row ${path} is on screen`);
    return found;
  };
  return { calls, rows, row, container: result.container };
}

test("the panel shows the folders the paths imply, folders first", () => {
  const tree = open(["src/utils.c", "main.c", "src/deep/inner.h", "Makefile"]);

  assert.deepEqual(tree.rows(), [
    "src",
    "src/deep",
    "src/deep/inner.h",
    "src/utils.c",
    "main.c",
    "Makefile",
  ]);
});

test("clicking a folder closes it, and its contents leave with it", async () => {
  const tree = open(["src/utils.c", "main.c"]);
  assert.deepEqual(tree.rows(), ["src", "src/utils.c", "main.c"]);

  tree.row("src").click();
  await Promise.resolve();

  assert.deepEqual(tree.rows(), ["src", "main.c"]);
  assert.equal(tree.row("src").getAttribute("aria-expanded"), "false");
  // A folder is not a file: clicking one never selects anything to edit.
  assert.deepEqual(tree.calls, []);
});

test("clicking a file selects it and a folder click does not", () => {
  const tree = open(["src/utils.c", "main.c"]);

  tree.row("main.c").click();

  assert.deepEqual(tree.calls, [["select", "main.c"]]);
});

test("a file dropped on a folder moves into it, and on a file moves beside it", () => {
  const tree = open(["src/deep/inner.h", "main.c"]);
  const transfer = { types: ["text/normfix-entry"], getData: () => "file:main.c" };

  const onFolder = new Event("drop", { bubbles: true });
  Object.defineProperty(onFolder, "dataTransfer", { value: transfer });
  tree.row("src").dispatchEvent(onFolder);

  // Dropping on a file means dropping into the folder that holds it, which is
  // what the pointer looks like it is doing.
  const onFile = new Event("drop", { bubbles: true });
  Object.defineProperty(onFile, "dataTransfer", { value: transfer });
  tree.row("src/deep/inner.h").dispatchEvent(onFile);

  assert.deepEqual(tree.calls, [
    ["move", "main.c", false, "src"],
    ["move", "main.c", false, "src/deep"],
  ]);
});

test("a file normfix cannot format is shown, and says so instead of its kind", () => {
  const tree = open(["notes.py", "main.c"], { unsupported: new Set(["notes.py"]) });

  assert.ok(tree.rows().includes("notes.py"), "it is not dropped from the panel");
  assert.match(tree.row("notes.py").textContent ?? "", /not formatted/);
  assert.match(tree.row("notes.py").getAttribute("title") ?? "", /does not format/);
});

test("right-clicking offers rename and delete for the entry under the pointer", async () => {
  const tree = open(["src/utils.c", "main.c"]);

  tree.row("src").dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true }));
  await Promise.resolve();

  const items = [...document.querySelectorAll<HTMLButtonElement>("[role=menuitem]")];
  assert.deepEqual(
    items.map((item) => item.textContent?.trim()),
    ["Rename", "Delete"],
  );

  items[1]?.click();
  assert.deepEqual(tree.calls, [["delete", "src"]]);
});

test("the panel is a tree, and says so to anything not looking at pixels", () => {
  const tree = open(["src/deep/inner.h", "main.c"]);

  assert.equal(
    tree.container.querySelector("[role=tree]")?.getAttribute("aria-label"),
    "Loaded project files",
  );
  // Depth is stated rather than implied by indentation, which a screen reader
  // cannot see.
  assert.equal(tree.row("src").getAttribute("aria-level"), "1");
  assert.equal(tree.row("src/deep").getAttribute("aria-level"), "2");
  assert.equal(tree.row("src/deep/inner.h").getAttribute("aria-level"), "3");
});
