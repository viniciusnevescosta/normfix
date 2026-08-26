import assert from "node:assert/strict";
import { test } from "vitest";

import {
  canonicalIdentityEmail,
  ImportBatchError,
  MAX_FILE_BYTES,
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

test("file reads overlap in a small bounded pool and preserve project order", async () => {
  let active = 0;
  let peak = 0;
  const candidates = Array.from({ length: 6 }, (_, index) => {
    const path = `${index}.c`;
    return [
      path,
      {
        arrayBuffer: async () => {
          active += 1;
          peak = Math.max(peak, active);
          await new Promise((resolve) => setTimeout(resolve, 5));
          active -= 1;
          return new TextEncoder().encode(`int value_${index};\n`).buffer;
        },
      },
    ] as const;
  });

  const imported = await readImportBatch(candidates, 0, () => 0);

  assert.equal(peak, 4);
  assert.deepEqual(
    [...imported.sources.keys()],
    candidates.map(([path]) => path),
  );
});

test("actual bytes are bounded even when candidate metadata understates the file", async () => {
  const revision = 1;
  const oversized = new Uint8Array(1024 * 1024 + 1).buffer;

  await assert.rejects(
    readImportBatch(
      [["large.c", { arrayBuffer: async () => oversized }]],
      revision,
      () => revision,
    ),
    (error: unknown) =>
      error instanceof ImportBatchError &&
      error.code === "file_too_large" &&
      error.path === "large.c",
  );
});

test("actual batch bytes cannot exceed the project budget", async () => {
  const candidates = Array.from({ length: 5 }, (_, index) => [
    `${index}.c`,
    { arrayBuffer: async () => new ArrayBuffer(MAX_FILE_BYTES) },
  ]) as Array<readonly [string, { arrayBuffer(): Promise<ArrayBuffer> }]>;

  await assert.rejects(
    readImportBatch(candidates, 0, () => 0),
    (error: unknown) => error instanceof ImportBatchError && error.code === "project_too_large",
  );
});
