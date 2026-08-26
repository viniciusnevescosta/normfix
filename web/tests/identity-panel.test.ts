// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import IdentityPanel from "../src/components/IdentityPanel.svelte";

function open(overrides: Record<string, unknown> = {}) {
  const calls: Array<[string, ...unknown[]]> = [];
  const { container } = render(IdentityPanel, {
    props: {
      email: "",
      stored: false,
      status: "",
      invalid: false,
      onSave: (email: string, remember: boolean) => calls.push(["save", email, remember]),
      onForget: () => calls.push(["forget"]),
      ...overrides,
    },
  });
  const button = (label: string): HTMLButtonElement | undefined =>
    [...container.querySelectorAll("button")].find((item) => item.textContent?.trim() === label);
  return { calls, container, button };
}

test("remembering a student identity is an explicit opt-in", () => {
  // This guarantee used to be checked by reading index.html for an unchecked
  // attribute. The panel is a component now, so it is checked where the
  // decision actually lives: an address typed and saved without touching the
  // box is kept for this tab only, and never written to the machine.
  const panel = open();
  const box = panel.container.querySelector<HTMLInputElement>("input[type=checkbox]");
  assert.ok(box, "the choice is offered");
  assert.equal(box.checked, false, "and it is not made for the reader");

  const field = panel.container.querySelector<HTMLInputElement>("#identity-email");
  assert.ok(field);
  field.value = "marvin@student.42.fr";
  field.dispatchEvent(new Event("input", { bubbles: true }));
  panel.button("Save")?.click();

  assert.deepEqual(panel.calls, [["save", "marvin@student.42.fr", false]]);
});

test("with nothing stored it offers to remember, and saves what was typed", () => {
  const panel = open();
  const field = panel.container.querySelector<HTMLInputElement>("#identity-email");
  const box = panel.container.querySelector<HTMLInputElement>("input[type=checkbox]");
  assert.ok(field && box, "the field and the box are both offered");

  field.value = "marvin@student.42.fr";
  field.dispatchEvent(new Event("input", { bubbles: true }));
  box.checked = true;
  box.dispatchEvent(new Event("change", { bubbles: true }));
  panel.button("Save")?.click();

  assert.deepEqual(panel.calls, [["save", "marvin@student.42.fr", true]]);
});

test("with an identity stored the box and Save are gone, leaving Forget", () => {
  const panel = open({ stored: true, email: "marvin@student.42.fr" });

  // The box would offer a choice already made, and Save would offer to make it
  // again. What is left is the one action that changes anything.
  assert.equal(panel.container.querySelector("input[type=checkbox]"), null);
  assert.equal(panel.button("Save"), undefined);

  panel.button("Forget")?.click();
  assert.deepEqual(panel.calls, [["forget"]]);
});

test("a refused address is marked for anything not reading the message", () => {
  const panel = open({ invalid: true, status: "Enter a valid 42 student email." });
  const field = panel.container.querySelector("#identity-email");

  assert.equal(field?.getAttribute("aria-invalid"), "true");
  assert.match(panel.container.textContent ?? "", /valid 42 student email/);
});
