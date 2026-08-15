import assert from "node:assert/strict";
import { test } from "vitest";

import { githubRequestInit, starCount } from "../src/github";

test("the repository request never serves a stale star count", () => {
  const init = githubRequestInit();

  // `force-cache` ignores expiry entirely, so a count read once would never
  // change again for that browser. GitHub sends max-age=60 with an ETag, and
  // the default mode is what respects it.
  assert.notEqual(init.cache, "force-cache");
  assert.notEqual(init.cache, "only-if-cached");
  assert.equal(init.cache, "default");
});

test("the repository request stays anonymous", () => {
  const init = githubRequestInit();

  assert.equal(init.credentials, "omit");
  assert.equal(init.referrerPolicy, "no-referrer");
});

test("only a plain non-negative count is displayed", () => {
  assert.equal(starCount({ stargazers_count: 0 }), 0);
  assert.equal(starCount({ stargazers_count: 1 }), 1);

  assert.equal(starCount({ stargazers_count: -1 }), null);
  assert.equal(starCount({ stargazers_count: 1.5 }), null);
  assert.equal(starCount({ stargazers_count: "12" }), null);
  assert.equal(starCount({}), null);
  assert.equal(starCount(null), null);
  assert.equal(starCount("nope"), null);
});
