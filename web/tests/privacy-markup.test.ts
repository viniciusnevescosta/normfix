import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "vitest";

test("remembering a student identity is an explicit opt-in", async () => {
  const html = await readFile(new URL("../index.html", import.meta.url), "utf8");
  const checkbox = html.match(/<input\s+id="remember-identity"[^>]*>/)?.[0];

  assert.ok(checkbox, "the identity persistence checkbox must exist");
  assert.doesNotMatch(checkbox, /\schecked(?:\s|>)/);
});
