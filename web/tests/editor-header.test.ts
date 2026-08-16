// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import EditorHeader from "../src/components/EditorHeader.svelte";

function open(props: Record<string, unknown>) {
  const { container } = render(EditorHeader, {
    props: {
      path: "main.c",
      lines: 8,
      bytes: 119,
      measure: (lines: number, bytes: number) => `${lines} lines · ${bytes} bytes`,
      label: "input",
      ...props,
    },
  });
  return container;
}

test("an open file is named with its size", () => {
  const header = open({});

  assert.match(header.textContent ?? "", /main\.c/);
  assert.match(header.textContent ?? "", /8 lines · 119 bytes/);
});

test("nothing open shows no name and no size", () => {
  // The imperative version cleared these from four places, so an empty title
  // beside a stale byte count was a state the page could reach.
  const header = open({ path: null });

  assert.ok(!(header.textContent ?? "").includes("lines"));
  assert.ok(!(header.textContent ?? "").includes("main.c"));
});
