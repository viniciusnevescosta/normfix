import assert from "node:assert/strict";
import { test } from "vitest";

import {
  MAX_STORED_BYTES,
  deserializeProject,
  isSameProject,
  serializeProject,
} from "../src/project/persistence";

const project = {
  files: { "main.c": "int main(void)\n{\n\treturn (0);\n}\n" },
  selected: "main.c",
  unsupported: ["notes.py"],
  savedAt: 1_700_000_000_000,
};

test("a project survives the round trip exactly", () => {
  const stored = serializeProject(project);
  assert.ok(stored);
  assert.deepEqual(deserializeProject(stored), project);
});

test("nothing worth restoring is not stored", () => {
  // Restoring nothing over nothing would only tell the reader their work came
  // back when it never went anywhere.
  assert.equal(serializeProject({ files: {}, selected: null, unsupported: [], savedAt: 1 }), null);

  // A project past the ceiling was imported rather than typed here, and is
  // recoverable from where it came from.
  const huge = { ...project, files: { "big.c": "x".repeat(MAX_STORED_BYTES + 1) } };
  assert.equal(serializeProject(huge), null);
});

test("damaged storage reads as absent rather than half a project", () => {
  assert.equal(deserializeProject(null), null);
  assert.equal(deserializeProject("not json"), null);
  assert.equal(deserializeProject("[]"), null);
  assert.equal(deserializeProject('{"files":null}'), null);
  assert.equal(deserializeProject('{"files":{}}'), null);

  // A file whose source is not text is dropped, and a project of nothing but
  // those is no project.
  assert.equal(deserializeProject('{"files":{"a.c":42}}'), null);
});

test("a restore that matches what is already open is not a restore", () => {
  const open = new Map(Object.entries(project.files));
  assert.ok(isSameProject(project, open));

  open.set("main.c", "different");
  assert.ok(!isSameProject(project, open));

  open.set("main.c", project.files["main.c"] ?? "");
  open.set("extra.c", "");
  assert.ok(!isSameProject(project, open));
});
