import assert from "node:assert/strict";
import test from "node:test";

import { translationCatalogueProblems } from "../src/i18n";

test("every advertised locale translates the complete browser catalogue", () => {
  assert.deepEqual(translationCatalogueProblems(), []);
});
