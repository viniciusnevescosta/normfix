// @vitest-environment happy-dom

import assert from "node:assert/strict";
import { afterEach, test, vi } from "vitest";

import type { PlaygroundElements } from "../src/dom";
import { createAppState, PROJECT_STORAGE_KEY } from "../src/playground/model";
import { createPersistenceController } from "../src/playground/persistence-controller";

afterEach(() => {
  vi.useRealTimers();
  localStorage.clear();
  document.body.replaceChildren();
});

test("discarding restored work does not schedule the sample back into storage", () => {
  vi.useFakeTimers();
  const discardRestore = document.createElement("button");
  const restoreNotice = document.createElement("div");
  const elements = { discardRestore, restoreNotice } as unknown as PlaygroundElements;
  const state = createAppState("en", "system");
  state.files = new Map([["secret.c", "int secret;\n"]]);
  state.folders = new Set(["secret/empty"]);
  let scheduleSave = (): void => {};
  const persistence = createPersistenceController({
    state,
    elements,
    translate: (key) => key,
    refreshRunControl: () => {},
    invalidateResults: () => scheduleSave(),
    selectFile: () => {},
  });
  scheduleSave = persistence.scheduleSave;
  localStorage.setItem(PROJECT_STORAGE_KEY, "stored work");
  persistence.attach();

  discardRestore.click();
  vi.advanceTimersByTime(1_000);

  assert.equal(localStorage.getItem(PROJECT_STORAGE_KEY), null);
  assert.deepEqual([...state.files.keys()], ["main.c"]);
  assert.deepEqual([...state.folders], []);
});
