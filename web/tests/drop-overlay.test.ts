// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import DropOverlay from "../src/components/DropOverlay.svelte";

test("nothing covers the page when nothing is being dragged", () => {
  const { container } = render(DropOverlay, {
    props: { active: false, translate: (key: string) => key },
  });

  assert.equal(container.textContent?.trim(), "");
});

test("a drag says what will be taken before anything is dropped", () => {
  // A drop that silently keeps four files out of seven is one the reader has
  // to reverse engineer afterwards.
  const { container } = render(DropOverlay, {
    props: { active: true, translate: (key: string) => key },
  });

  assert.match(container.textContent ?? "", /Drop files or a project folder/);
  assert.match(container.textContent ?? "", /Takes \.c, \.h, \.md, and Makefile/);
});
