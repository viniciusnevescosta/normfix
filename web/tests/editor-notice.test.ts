// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import EditorNotice from "../src/components/EditorNotice.svelte";

test("nothing covers the editor when there is nothing to say", () => {
  // The imperative version set `hidden` and a `display` rule kept the element
  // on screen anyway, twice. There is no attribute to disagree with now.
  const { container } = render(EditorNotice, { props: { notice: null } });

  assert.equal(container.textContent?.trim(), "");
  assert.equal(container.querySelector("[role=status]"), null);
});

test("a notice covers the editor and says both halves", () => {
  const { container } = render(EditorNotice, {
    props: {
      notice: {
        title: "normfix does not format this file.",
        detail: "It formats .c, .h, .md, and Makefile.",
      },
    },
  });

  const cover = container.querySelector("[role=status]");
  assert.ok(cover, "the editor is covered rather than left looking editable");
  assert.match(cover.textContent ?? "", /does not format this file/);
  assert.match(cover.textContent ?? "", /It formats \.c, \.h, \.md, and Makefile/);
});
