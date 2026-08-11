import assert from "node:assert/strict";
import test from "node:test";

import {
  ImportBatchError,
  buildTar,
  canonicalIdentityEmail,
  portablePathKey,
  readImportBatch,
  sourcePathProblem,
} from "../src/project/files";

const decoder = new TextDecoder();

function field(archive: Uint8Array, start: number, length: number): string {
  return decoder.decode(archive.slice(start, start + length)).replace(/\0.*$/s, "");
}

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
  assert.equal(
    canonicalIdentityEmail("  Student-A@STUDENT.42.FR "),
    "student-a@student.42.fr",
  );
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
    (error: unknown) =>
      error instanceof ImportBatchError && error.code === "project_changed",
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

  assert.deepEqual([...imported.sources], [
    ["a.c", "int a;\n"],
    ["b.h", "int b;\n"],
  ]);
  assert.equal(imported.selectedPath, "b.h");
});

test("the generated archive is a valid deterministic ustar record", () => {
  const archive = buildTar([{ path: "src/main.c", source: "int x;\n" }]);
  assert.equal(archive.length, 2048);
  assert.equal(field(archive, 0, 100), "src/main.c");
  assert.equal(field(archive, 257, 6), "ustar");
  assert.equal(decoder.decode(archive.slice(512, 519)), "int x;\n");
  assert.ok(archive.slice(1024).every((byte) => byte === 0));

  const storedChecksum = Number.parseInt(field(archive, 148, 8).trim(), 8);
  const header = archive.slice(0, 512);
  header.fill(32, 148, 156);
  const computedChecksum = header.reduce((total, byte) => total + byte, 0);
  assert.equal(storedChecksum, computedChecksum);
});

test("long portable paths use the ustar prefix field without truncation", () => {
  const prefix = `src/${"nested/".repeat(13)}`.replace(/\/$/, "");
  const archive = buildTar([{ path: `${prefix}/main.c`, source: "" }]);

  assert.equal(field(archive, 0, 100), "main.c");
  assert.equal(field(archive, 345, 155), prefix);
});
