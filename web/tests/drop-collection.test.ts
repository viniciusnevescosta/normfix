import assert from "node:assert/strict";
import { test } from "vitest";

import { captureDroppedEntries, collectDroppedFiles } from "../src/project/drop";
import { MAX_UNSUPPORTED_FILES } from "../src/project/files";

test("a single huge dropped directory stops at the global scan budget", async () => {
  const children = Array.from({ length: 20_050 }, (_, index) => ({
    name: `${index}.o`,
    fullPath: `/build/${index}.o`,
    isFile: true,
    isDirectory: false,
  }));
  let reads = 0;
  const directory = {
    name: "build",
    fullPath: "/build",
    isFile: false,
    isDirectory: true,
    createReader: () => ({
      readEntries: (success: (entries: typeof children) => void) => {
        reads += 1;
        success(reads === 1 ? children : []);
      },
    }),
  };

  const selection = await collectDroppedFiles([directory]);

  assert.equal(reads, 1, "the walker does not drain entries beyond its safety ceiling");
  assert.equal(selection.files.length, 0);
  assert.deepEqual(selection.folders, ["build"]);
  assert.equal(selection.unsupported.length, MAX_UNSUPPORTED_FILES);
  assert.ok(selection.skipped > 0, "the cutoff is reported rather than silently discarded");
});

test("a dropped empty directory remains visible to the project", async () => {
  const directory = {
    name: "empty",
    fullPath: "/project/empty",
    isFile: false,
    isDirectory: true,
    createReader: () => ({
      readEntries: (success: (entries: never[]) => void) => success([]),
    }),
  };

  const selection = await collectDroppedFiles([directory]);

  assert.deepEqual(selection.files, []);
  assert.deepEqual(selection.folders, ["project/empty"]);
  assert.equal(selection.skipped, 0);
});

test("a browser without the entry extension falls back to plain dropped files", () => {
  const entry = { name: "main.c" } as FileSystemEntry;
  assert.deepEqual(captureDroppedEntries([{} as DataTransferItem]), []);
  assert.deepEqual(
    captureDroppedEntries([
      { webkitGetAsEntry: () => entry } as unknown as DataTransferItem,
      {
        webkitGetAsEntry: () => {
          throw new Error("not implemented");
        },
      } as unknown as DataTransferItem,
    ]),
    [entry],
  );
});
