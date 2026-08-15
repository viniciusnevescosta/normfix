import assert from "node:assert/strict";
import { test } from "vitest";

import { THEME_PREFERENCES, isThemePreference, resolveAppearance } from "../src/theme";

test("an explicit choice overrides the system in both directions", () => {
  // The point of offering the override is that it wins. A reader on a light
  // desktop who picks dark must get dark, and the reverse.
  assert.equal(resolveAppearance("dark", true), "dark");
  assert.equal(resolveAppearance("light", false), "light");
});

test("following the system means following it", () => {
  assert.equal(resolveAppearance("system", true), "light");
  assert.equal(resolveAppearance("system", false), "dark");
});

test("a browser that reports nothing gets the tested appearance", () => {
  // matchMedia returns false for an unsupported query, which is
  // indistinguishable from "prefers dark". Falling back to dark means that
  // case lands on the palette the playground was designed in, rather than on
  // a light theme chosen by accident.
  assert.equal(resolveAppearance("system", false), "dark");
});

test("only the three published preferences are accepted", () => {
  for (const preference of THEME_PREFERENCES) {
    assert.ok(isThemePreference(preference), preference);
  }
  // Storage is device-local and editable; a stale or hand-written value must
  // fall back rather than end up on the root element as an attribute.
  assert.ok(!isThemePreference("solarized"));
  assert.ok(!isThemePreference(""));
  assert.ok(!isThemePreference(null));
  assert.ok(!isThemePreference(undefined));
});
