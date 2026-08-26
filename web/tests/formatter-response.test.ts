import assert from "node:assert/strict";
import { test } from "vitest";

import { FormatterResponseError, parseFormatterResponse } from "../src/formatter-response";

function file(path: string) {
  return {
    path,
    formatted: "int main(void);\n",
    changed: true,
    stable: true,
    fixes: [],
    diagnostics: [],
    budget: [],
    diff: "",
    error: null,
  };
}

test("the wasm boundary derives its summary from validated files", () => {
  const response = parseFormatterResponse(
    JSON.stringify({
      schema_version: 1,
      files: [file("main.c")],
      summary: { files: 999, changed: 999, fixes: 999, diagnostics: 999, failed: 999 },
    }),
    new Map([["main.c", "int main(void);\n"]]),
  );

  assert.deepEqual(response.summary, {
    files: 1,
    changed: 1,
    fixes: 0,
    diagnostics: 0,
    failed: 0,
  });
});

test("unknown, missing, and duplicate result paths are refused", () => {
  const inputs = new Map([["main.c", ""]]);
  for (const files of [[file("other.c")], [], [file("main.c"), file("main.c")]]) {
    assert.throws(
      () => parseFormatterResponse(JSON.stringify({ schema_version: 1, files }), inputs),
      FormatterResponseError,
    );
  }
});

test("malformed nested findings do not reach reactive UI state", () => {
  const malformed = file("main.c");
  (malformed as unknown as { diagnostics: unknown }).diagnostics = [
    { rule_id: "RULE", severity: "critical", message: "bad" },
  ];

  assert.throws(
    () =>
      parseFormatterResponse(
        JSON.stringify({ schema_version: 1, files: [malformed] }),
        new Map([["main.c", ""]]),
      ),
    (error: unknown) => error instanceof FormatterResponseError && error.code === "schema",
  );
});

test("the wasm response cannot expand the browser project past its file budget", () => {
  const files = Array.from({ length: 129 }, (_, index) => file(`file-${index}.c`));
  const inputs = new Map(files.map((entry) => [entry.path, ""]));

  assert.throws(
    () => parseFormatterResponse(JSON.stringify({ schema_version: 1, files }), inputs),
    (error: unknown) => error instanceof FormatterResponseError && error.code === "schema",
  );
});
