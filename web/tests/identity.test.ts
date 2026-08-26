// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { afterEach, test } from "vitest";

import {
  chooseIdentity,
  hasStoredIdentity,
  IDENTITY_STORAGE_KEY,
  loadBrowserIdentity,
  removeStoredIdentity,
} from "../src/identity";

afterEach(() => localStorage.clear());

test("identity persistence is an explicit choice, never a side effect of validation", () => {
  const session = chooseIdentity("marvin@student.42.fr", false);
  assert.deepEqual(session, { email: "marvin@student.42.fr", outcome: "session" });
  assert.equal(localStorage.getItem(IDENTITY_STORAGE_KEY), null);

  const saved = chooseIdentity("marvin@student.42.fr", true);
  assert.deepEqual(saved, { email: "marvin@student.42.fr", outcome: "saved" });
  assert.equal(hasStoredIdentity(), true);
  assert.equal(loadBrowserIdentity(), "marvin@student.42.fr");

  removeStoredIdentity();
  assert.equal(hasStoredIdentity(), false);
});

test("invalid stored identity is removed instead of entering app state", () => {
  localStorage.setItem(IDENTITY_STORAGE_KEY, "person@example.com");

  assert.equal(loadBrowserIdentity(), null);
  assert.equal(localStorage.getItem(IDENTITY_STORAGE_KEY), null);
  assert.deepEqual(chooseIdentity("person@example.com", true), {
    email: null,
    outcome: "invalid",
  });
});
