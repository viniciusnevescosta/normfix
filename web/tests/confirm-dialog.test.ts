// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import ConfirmDialog from "../src/components/ConfirmDialog.svelte";

function open(request: { text: string } | null) {
  const calls: string[] = [];
  const { container } = render(ConfirmDialog, {
    props: {
      request,
      onConfirm: () => calls.push("confirm"),
      onCancel: () => calls.push("cancel"),
    },
  });
  const button = (label: string): HTMLButtonElement | undefined =>
    [...container.querySelectorAll("button")].find((item) => item.textContent?.trim() === label);
  return { calls, container, button };
}

test("nothing is asked when nothing is being deleted", () => {
  const dialog = open(null);

  assert.equal(dialog.container.querySelector("h2"), null);
});

test("confirming deletes and cancelling does not", () => {
  // The old wiring went through the dialog's `close` event and return value,
  // and confirming quietly did nothing. There is one path now.
  const asking = open({ text: "Delete src and the 2 file(s) in it?" });
  assert.match(asking.container.textContent ?? "", /Delete src and the 2/);

  asking.button("Delete")?.click();
  assert.deepEqual(asking.calls, ["confirm"]);

  const again = open({ text: "Delete main.c?" });
  again.button("Cancel")?.click();
  assert.deepEqual(again.calls, ["cancel"]);
});
