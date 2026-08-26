import assert from "node:assert/strict";
import { test } from "vitest";

import { MAX_UNSUPPORTED_FILES } from "../src/project/files";
import { ImportPlanError, planImport } from "../src/project/import-plan";

const file = (size = 1) => ({ size, arrayBuffer: async () => new ArrayBuffer(size) });

test("an import plan is atomic and keeps only portable warning paths", () => {
  const plan = planImport(
    [
      { path: "src/main.c", file: file() },
      { path: "build/app.o", file: file() },
      { path: "../outside.o", file: file() },
    ],
    ["notes.txt", "/absolute.bin"],
    ["open.c"],
    [],
  );

  assert.deepEqual(
    [...plan.candidates.values()].map(([path]) => path),
    ["src/main.c"],
  );
  assert.deepEqual([...plan.unsupported], ["notes.txt", "build/app.o"]);
  assert.equal(plan.ignored, 4);
  assert.equal(plan.firstRejected, "/absolute.bin");
});

test("the visible warning list is bounded independently from the dropped tree", () => {
  const paths = Array.from({ length: MAX_UNSUPPORTED_FILES + 50 }, (_, index) => `obj/${index}.o`);
  const plan = planImport([], paths, [], []);

  assert.equal(plan.unsupported.size, MAX_UNSUPPORTED_FILES);
  assert.equal(plan.ignored, paths.length);
});

test("portable duplicate and loaded paths fail before any file is read", () => {
  assert.throws(
    () =>
      planImport(
        [
          { path: "SRC/Main.C", file: file() },
          { path: "src/main.c", file: file() },
        ],
        [],
        [],
        [],
      ),
    (error: unknown) => error instanceof ImportPlanError && error.code === "duplicate",
  );

  assert.throws(
    () => planImport([{ path: "MAIN.c", file: file() }], [], ["main.c"], []),
    (error: unknown) => error instanceof ImportPlanError && error.code === "conflict",
  );
});
