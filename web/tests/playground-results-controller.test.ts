// @vitest-environment happy-dom

import assert from "node:assert/strict";
import { test } from "vitest";

import type { PlaygroundElements } from "../src/dom";
import type { SourceEditor } from "../src/editor";
import { createAppState, type ResultRecord } from "../src/playground/model";
import { createResultsController } from "../src/playground/results-controller";

test("typing after findings are cleared does not repaint every Monaco model", () => {
  const state = createAppState("en", "system");
  let markerPaints = 0;
  state.editor = {
    usingMonaco: true,
    getValue: () => "",
    setFile: () => {},
    removeFile: () => {},
    focus: () => {},
    setAppearance: () => {},
    setMarkers: () => {
      markerPaints += 1;
    },
  } satisfies SourceEditor;
  const resultsElement = document.createElement("section");
  const controller = createResultsController({
    state,
    elements: { results: resultsElement } as unknown as PlaygroundElements,
    translate: (key) => key,
    translatePlural: (base, count) => `${base}:${count}`,
    setRuntime: () => {},
    setRuntimeMessage: () => {},
    refreshRunControl: () => {},
    scheduleSave: () => {},
    syncEditor: () => {},
    validateSources: () => {},
    renderFileList: () => {},
    selectFile: () => {},
  });

  controller.invalidateResults();
  assert.equal(markerPaints, 0);

  const result: ResultRecord = {
    path: "main.c",
    inputSource: state.files.get("main.c") ?? "",
    formatted: "",
    changed: true,
    stable: true,
    fixes: [],
    diagnostics: [],
    budget: [],
    diff: "",
    error: null,
  };
  state.results.set(result.path, result);
  state.selectedResult = result.path;
  controller.invalidateResults();
  controller.invalidateResults();

  assert.equal(markerPaints, 1);
  assert.equal(resultsElement.hidden, true);
});
