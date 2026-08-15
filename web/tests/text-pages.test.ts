import assert from "node:assert/strict";
import { test } from "vitest";

// @ts-expect-error - the build script is plain ESM without types.
import { plainText, routeFor } from "../../docs/.vitepress/build-text-pages.mjs";

test("frontmatter is dropped and the prose is kept", () => {
  const source =
    "---\nlayout: home\ntitle: Why\n---\n\n# Why normfix\n\nIt changes only what it can prove.\n";

  assert.equal(plainText(source), "# Why normfix\n\nIt changes only what it can prove.\n");
});

test("a container fence goes and the sentence inside it stays", () => {
  // The body of a callout is the part a page marks as most important. Dropping
  // it with the fence would lose exactly the sentences that matter most.
  const source = [
    "# Undo",
    "",
    "::: warning",
    "A run that was never backed up cannot be undone.",
    ":::",
    "",
    "Then run it again.",
  ].join("\n");

  const text = plainText(source);
  assert.match(text, /A run that was never backed up cannot be undone\./);
  assert.doesNotMatch(text, /:::/);
  assert.match(text, /Then run it again\./);
});

test("a code block keeps its code and loses its editor label", () => {
  const source = "```sh [terminal]\nnormfix --check\n```\n";

  const text = plainText(source);
  assert.match(text, /normfix --check/);
  assert.doesNotMatch(text, /\[terminal\]/);
});

test("a page is published at its own route plus .txt", () => {
  assert.equal(routeFor("guide/getting-started.md"), "guide/getting-started");
  assert.equal(routeFor("pt/commands/leaks.md"), "pt/commands/leaks");
  assert.equal(routeFor("index.md"), "index");
});
