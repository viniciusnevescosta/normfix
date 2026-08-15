import assert from "node:assert/strict";
import { test } from "vitest";

import { decodeUtf8Source } from "../src/project/source-text";

test("a leading UTF-8 BOM is consumed before header processing", () => {
  const bytes = Uint8Array.from([0xef, 0xbb, 0xbf, 0x69, 0x6e, 0x74]).buffer;
  assert.equal(decodeUtf8Source(bytes), "int");
});

test("malformed UTF-8 is rejected instead of replaced", () => {
  const bytes = Uint8Array.from([0xc3, 0x28]).buffer;
  assert.throws(() => decodeUtf8Source(bytes), TypeError);
});
