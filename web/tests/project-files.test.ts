import assert from "node:assert/strict";
import { test } from "vitest";

import {
  ImportBatchError,
  canonicalIdentityEmail,
  portablePathKey,
  readImportBatch,
  sourcePathProblem,
} from "../src/project/files";

test("portable paths are accepted byte-for-byte without silent normalization", () => {
  assert.equal(sourcePathProblem("src/main.c"), null);
  assert.equal(sourcePathProblem("README.md"), null);
  assert.equal(sourcePathProblem("makefile"), null);

  for (const path of ["./main.c", "src\\main.c", "src//main.c", "src/../main.c"]) {
    assert.equal(sourcePathProblem(path)?.code, "portable_path", path);
  }
});

test("portable path keys detect collisions without changing the displayed path", () => {
  assert.equal(portablePathKey("SRC/Main.C"), portablePathKey("src/main.c"));
  assert.notEqual("SRC/Main.C", "src/main.c");
});

test("42 identity validation canonicalizes supported addresses and rejects impostors", () => {
  assert.equal(canonicalIdentityEmail("  Student-A@STUDENT.42.FR "), "student-a@student.42.fr");
  assert.equal(canonicalIdentityEmail("student-a@example.com"), null);
  assert.equal(canonicalIdentityEmail("bad%login@student.42.fr"), null);
});

test("an import is discarded when the project changes during an asynchronous read", async () => {
  let revision = 4;
  let finishRead: ((value: ArrayBuffer) => void) | undefined;
  const read = new Promise<ArrayBuffer>((resolve) => {
    finishRead = resolve;
  });
  const pending = readImportBatch(
    [["src/new.c", { arrayBuffer: () => read }]],
    revision,
    () => revision,
  );

  await Promise.resolve();
  revision += 1;
  finishRead?.(new TextEncoder().encode("int value;\n").buffer);

  await assert.rejects(
    pending,
    (error: unknown) => error instanceof ImportBatchError && error.code === "project_changed",
  );
});

test("an import batch returns decoded sources only after every read succeeds", async () => {
  const revision = 2;
  const imported = await readImportBatch(
    [
      ["a.c", { arrayBuffer: async () => new TextEncoder().encode("int a;\n").buffer }],
      ["b.h", { arrayBuffer: async () => new TextEncoder().encode("int b;\n").buffer }],
    ],
    revision,
    () => revision,
  );

  assert.deepEqual(
    [...imported.sources],
    [
      ["a.c", "int a;\n"],
      ["b.h", "int b;\n"],
    ],
  );
  assert.equal(imported.selectedPath, "b.h");
});
