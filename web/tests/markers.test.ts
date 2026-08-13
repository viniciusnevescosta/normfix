import assert from "node:assert/strict";
import test from "node:test";

import { markersFor } from "../src/project/markers";

test("a finding with a position becomes a mark carrying its rule", () => {
  const markers = markersFor([
    {
      rule_id: "SPACE_REPLACE_TAB",
      severity: "error",
      message: "Found spaces when expecting a tab",
      location: { line: 7, column: 2 },
    },
  ]);

  assert.deepEqual(markers, [
    {
      severity: "error",
      message: "Found spaces when expecting a tab",
      ruleId: "SPACE_REPLACE_TAB",
      line: 7,
      column: 2,
    },
  ]);
});

test("a finding about the whole file is never drawn under a line", () => {
  // An invalid header belongs to the file, not to a position in it. Marking it
  // anywhere would put a squiggle under code that has nothing to do with it.
  const markers = markersFor([
    {
      rule_id: "INVALID_HEADER",
      severity: "warning",
      message: "The official 42 header is missing or malformed",
      location: null,
    },
  ]);

  assert.deepEqual(markers, []);
});

test("a position before the start of the document is pulled back inside it", () => {
  const markers = markersFor([
    {
      rule_id: "PARSE_RECOVERY",
      severity: "error",
      message: "unreadable",
      location: { line: 0, column: 0 },
    },
  ]);

  assert.equal(markers[0]?.line, 1);
  assert.equal(markers[0]?.column, 1);
});
