// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import StatusBadges from "../src/components/StatusBadges.svelte";

function open(overrides: Record<string, unknown> = {}) {
  const calls: string[] = [];
  const { container } = render(StatusBadges, {
    props: {
      runtime: "ready",
      runtimeLabel: "ready",
      offline: "active",
      online: true,
      onUpdate: () => calls.push("update"),
      ...overrides,
    },
  });
  return { calls, container };
}

test("offline support offers nothing while it is simply working", () => {
  // A badge that is always talking is one nobody reads.
  const badges = open();

  assert.equal(badges.container.querySelector("button"), null);
  assert.match(badges.container.textContent ?? "", /[Oo]ffline/);
});

test("an update waiting to be taken is the one thing it interrupts for", () => {
  const badges = open({ offline: "update-ready" });

  const action = badges.container.querySelector("button");
  assert.ok(action, "the reader is given the way to take it");
  assert.match(badges.container.textContent ?? "", /[Nn]ew version/);

  action.click();
  assert.deepEqual(badges.calls, ["update"]);
});

test("the formatter's state is carried where a stylesheet can see it", () => {
  const badges = open({ runtime: "error", runtimeLabel: "formatterFailed" });

  assert.equal(
    badges.container.querySelector("[data-state=error]")?.getAttribute("role"),
    "status",
  );
  assert.match(badges.container.textContent ?? "", /formatterFailed/);
});
