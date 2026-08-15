import assert from "node:assert/strict";
import { test } from "vitest";

import {
  buildTree,
  movedPath,
  renamedPath,
  rewritePrefix,
  wouldContainItself,
} from "../src/project/tree";

test("a flat set of paths becomes the folders it implies", () => {
  const tree = buildTree(["src/utils.c", "main.c", "src/deep/inner.h", "Makefile"]);

  // Folders first, then files, each group by name: what a file browser shows,
  // rather than the order a map happened to hold.
  assert.deepEqual(
    tree.map((node) => `${node.kind}:${node.name}`),
    ["folder:src", "file:main.c", "file:Makefile"],
  );

  const source = tree[0];
  assert.equal(source?.kind, "folder");
  if (source?.kind !== "folder") return;
  assert.deepEqual(
    source.children.map((node) => `${node.kind}:${node.name}`),
    ["folder:deep", "file:utils.c"],
  );
  assert.equal(source.path, "src", "a folder carries the prefix it stands for");
});

test("a file dropped on a folder keeps its name and takes the folder's prefix", () => {
  assert.equal(movedPath("main.c", "src"), "src/main.c");
  assert.equal(movedPath("src/deep/inner.h", "src"), "src/inner.h");
  // The project root is the empty prefix, which is how a file comes back out.
  assert.equal(movedPath("src/utils.c", ""), "utils.c");
  // Dropping something where it already is has to be recognisable as no move.
  assert.equal(movedPath("src/utils.c", "src"), "src/utils.c");
});

test("a folder cannot be dropped inside itself", () => {
  assert.ok(wouldContainItself("src", "src/deep"));
  assert.ok(wouldContainItself("src", "src"));
  assert.ok(!wouldContainItself("src", "tests"));
  // A name that merely starts the same is a different folder.
  assert.ok(!wouldContainItself("src", "srcs/deep"));
});

test("renaming a folder rewrites the prefix of everything beneath it", () => {
  const paths = ["src/utils.c", "src/deep/inner.h", "srcs/other.c", "main.c"];

  assert.deepEqual(rewritePrefix(paths, "src", "lib"), [
    ["src/utils.c", "lib/utils.c"],
    ["src/deep/inner.h", "lib/deep/inner.h"],
  ]);

  // `srcs/` shares four letters with `src` and is not inside it.
  assert.equal(renamedPath("src/utils.c", "helpers.c"), "src/helpers.c");
  assert.equal(renamedPath("main.c", "start.c"), "start.c");
});
