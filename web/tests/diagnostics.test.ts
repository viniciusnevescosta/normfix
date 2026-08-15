// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import Diagnostics from "../src/components/Diagnostics.svelte";

const finding = {
  rule_id: "C_SYNTAX_RECOVERY",
  severity: "warning",
  message: "The C parser recovered around syntax node `ERROR`.",
  location: { line: 1, column: 10 },
  help: "Repair the malformed construct, then rerun normfix.",
  source: "native",
};

function open(overrides: Record<string, unknown> = {}) {
  const { container } = render(Diagnostics, {
    props: {
      diagnostics: [],
      fixes: [],
      budget: [],
      error: null,
      stable: true,
      translate: (key: string, values?: Record<string, string | number>) =>
        values ? `${key}:${Object.values(values).join(",")}` : key,
      ...overrides,
    },
  });
  return { container, text: () => container.textContent ?? "" };
}

test("a clean run says so rather than showing an empty panel", () => {
  const panel = open();

  assert.match(panel.text(), /noDiagnostics/);
  assert.match(panel.text(), /cliCoverage/);
});

test("a file that would not parse shows the reason and then the finding", () => {
  const panel = open({
    error: "automatic edits require a lossless C parse",
    stable: false,
    diagnostics: [finding],
  });

  // The reason comes first, and the finding is not hidden behind it: where the
  // parser lost its way is the one thing worth acting on.
  assert.match(panel.text(), /unparsableFile/);
  assert.match(panel.text(), /C_SYNTAX_RECOVERY/);
  assert.match(panel.text(), /L1:C10/);
  assert.ok(!panel.text().includes("unstableFormatter"), "an error is not an unstable run");
});

test("an unstable run is told apart from an unreadable file", () => {
  // Both arrive with `stable: false`; only one carries an error, which is what
  // separates them.
  const panel = open({ error: null, stable: false });

  assert.match(panel.text(), /unstableFormatter/);
  assert.ok(!panel.text().includes("unparsableFile"));
});

test("what was fixed and how much room is left are both reported", () => {
  const panel = open({
    fixes: [{ rule_id: "REPLACE_TERNARY", description: "replaced a forbidden ternary" }],
    budget: [
      {
        function: "process",
        line: 12,
        lines: 31,
        line_limit: 25,
        variables: 3,
        variable_limit: 5,
        parameters: 2,
        parameter_limit: 4,
      },
    ],
  });

  assert.match(panel.text(), /fixesApplied:1/);
  assert.match(panel.text(), /REPLACE_TERNARY/);
  assert.match(panel.text(), /process\(\)/);

  // A function over its limit is marked, rather than leaving the reader to
  // compare two numbers in every row.
  const over = [...panel.container.querySelectorAll("td")].filter((cell) =>
    cell.classList.contains("text-error"),
  );
  assert.deepEqual(
    over.map((cell) => cell.textContent?.trim()),
    ["31/25"],
  );
});
