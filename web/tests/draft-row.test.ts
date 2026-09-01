// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { test } from "vitest";

import { openDraftRow } from "../src/project/draft-row";

function press(input: Element, key: string): void {
  input.dispatchEvent(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

function draft(container: Element): HTMLInputElement {
  const input = container.querySelector<HTMLInputElement>(".file-draft input");
  assert.ok(input, "a draft row is open");
  return input;
}

function list(): HTMLElement {
  document.body.replaceChildren();
  const container = document.createElement("div");
  document.body.append(container);
  return container;
}

test("a typed name commits on Enter and the row goes away", () => {
  const container = list();
  const created: string[] = [];
  openDraftRow({
    container,
    kind: "file",
    label: "New file",
    create: (path) => created.push(path),
  });

  const input = draft(container);
  input.value = "utils.c";
  press(input, "Enter");

  assert.deepEqual(created, ["utils.c"]);
  assert.equal(container.querySelector(".file-draft"), null);
});

test("naming a folder creates it without forcing a file", () => {
  const container = list();
  const created: string[] = [];
  openDraftRow({
    container,
    kind: "folder",
    label: "New folder",
    create: (path) => created.push(path),
  });

  const folder = draft(container);
  folder.value = "src";
  press(folder, "Enter");

  assert.deepEqual(created, ["src"]);
  assert.equal(container.querySelector(".file-draft"), null);
});

test("a refused name keeps the row open with what was typed", () => {
  const container = list();
  openDraftRow({
    container,
    kind: "file",
    label: "New file",
    create: () => {
      throw new Error("Only .c, .h, .md and Makefile are accepted.");
    },
  });

  const input = draft(container);
  input.value = "notes.py";
  press(input, "Enter");

  assert.equal(input.value, "notes.py", "the reader does not retype a name to fix it");
  assert.match(container.querySelector(".file-draft-error")?.textContent ?? "", /Only \.c/);

  // Clicking away while the refusal is on screen would take the explanation
  // and the typed name with it.
  input.dispatchEvent(new FocusEvent("blur"));
  assert.ok(container.querySelector(".file-draft"), "the row stays while it is explaining itself");
});

test("Escape and an empty name abandon the row without creating", () => {
  const container = list();
  const created: string[] = [];
  openDraftRow({
    container,
    kind: "file",
    label: "New file",
    create: (path) => created.push(path),
  });
  press(draft(container), "Escape");
  assert.equal(container.querySelector(".file-draft"), null);

  openDraftRow({
    container,
    kind: "file",
    label: "New file",
    create: (path) => created.push(path),
  });
  const empty = draft(container);
  empty.value = "   ";
  press(empty, "Enter");

  assert.deepEqual(created, []);
  assert.equal(container.querySelector(".file-draft"), null);
});
