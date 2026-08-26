import assert from "node:assert/strict";
import { test } from "vitest";

import type { MessageKey } from "../src/i18n";
import {
  countFolderEntries,
  editorMeasurements,
  hasPortablePath,
  validateProjectSources,
} from "../src/playground/project-model";

const translate = (
  key: MessageKey,
  values: Readonly<Record<string, string | number>> = {},
): string => `${key}:${JSON.stringify(values)}`;

test("a new source cannot shadow a portable alias", () => {
  assert.equal(hasPortablePath(["src/APP.C"], "SRC/app.c"), true);
  assert.equal(hasPortablePath(["main.c"], "src/app.c"), false);
});

test("project validation rejects portable aliases before formatting", () => {
  assert.throws(
    () =>
      validateProjectSources(
        new Map([
          ["SRC/Main.C", "int main(void);\n"],
          ["src/main.c", "int main(void);\n"],
        ]),
        translate,
      ),
    /pathCollision/,
  );
});

test("editor measurements count UTF-8 bytes rather than UTF-16 code units", () => {
  assert.deepEqual(editorMeasurements("é\n"), { lines: 2, bytes: 3 });
  assert.deepEqual(editorMeasurements(""), { lines: 0, bytes: 0 });
});

test("a folder deletion count includes files normfix only warns about", () => {
  assert.equal(
    countFolderEntries(["src/main.c", "include/lib.h"], ["src/main.o", "notes.txt"], "src"),
    2,
  );
});
