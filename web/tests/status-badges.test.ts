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

test("offline support says nothing at all while it is simply working", () => {
  // A badge that is always talking is one nobody reads, so it is absent rather
  // than reassuring — the formatter's badge is then the only one on screen.
  const badges = open();

  assert.equal(badges.container.querySelector("button"), null);
  assert.ok(!/offline/i.test(badges.container.textContent ?? ""));
  // And with the formatter ready, the top bar carries no badge at all.
  assert.equal(badges.container.textContent?.trim(), "");
});

test("the formatter speaks while it is coming and when it failed to come", () => {
  // Ready is the state the reader expects and can do nothing about.
  assert.match(
    open({ runtime: "loading", runtimeLabel: "loading" }).container.textContent ?? "",
    /loading/,
  );
  assert.match(
    open({ runtime: "error", runtimeLabel: "failed" }).container.textContent ?? "",
    /failed/,
  );
  assert.equal(open({ runtime: "ready" }).container.textContent?.trim(), "");
});

test("being offline without support is one of the two things it speaks for", () => {
  const badges = open({ offline: "ready", online: false });

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
