// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import CodeView from "../src/components/CodeView.svelte";

test("the text is shown exactly, including what looks like markup", () => {
  // Source is text, not HTML: a header comment full of slashes and asterisks
  // has to arrive as the reader wrote it.
  const source = "#include <unistd.h>\n\nint\tmain(void)\n{\n\treturn (0);\n}\n";
  const { container } = render(CodeView, { props: { text: source } });

  assert.equal(container.querySelector("code")?.textContent, source);
  assert.equal(container.querySelector("code")?.querySelector("*"), null);
});

test("the element holding the text is handed to whoever needs to select it", () => {
  let given: HTMLElement | null = null;
  render(CodeView, { props: { text: "x", bind: (element: HTMLElement) => (given = element) } });

  assert.ok(given, "the copy path is given the element rather than looking it up");
  assert.equal((given as unknown as HTMLElement).tagName, "CODE");
});
