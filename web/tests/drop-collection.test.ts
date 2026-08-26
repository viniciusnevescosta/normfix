import assert from "node:assert/strict";
import { test } from "vitest";

import { collectDroppedFiles } from "../src/project/drop";
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
  assert.equal(selection.unsupported.length, MAX_UNSUPPORTED_FILES);
  assert.ok(selection.skipped > 0, "the cutoff is reported rather than silently discarded");
});
