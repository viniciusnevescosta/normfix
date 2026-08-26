// @vitest-environment happy-dom
import assert from "node:assert/strict";
import { render } from "@testing-library/svelte";
import { test } from "vitest";

import TopBar from "../src/components/TopBar.svelte";

function open(overrides: Record<string, unknown> = {}) {
  const calls: Array<[string, string]> = [];
  const { container } = render(TopBar, {
    props: {
      locale: "pt",
      theme: "system",
      docsHref: "/docs/pt/",
      onLocale: (locale: string) => calls.push(["locale", locale]),
      onTheme: (theme: string) => calls.push(["theme", theme]),
      ...overrides,
    },
  });
  return { calls, container };
}

test("the pickers show what is chosen and report a change", () => {
  const bar = open();
  const [language, appearance] = [...bar.container.querySelectorAll("select")];
  assert.ok(language && appearance);
  assert.equal(language.value, "pt");
  assert.equal(appearance.value, "system");

  language.value = "fr";
  language.dispatchEvent(new Event("change", { bubbles: true }));
  appearance.value = "dark";
  appearance.dispatchEvent(new Event("change", { bubbles: true }));

  assert.deepEqual(bar.calls, [
    ["locale", "fr"],
    ["theme", "dark"],
  ]);
});

test("opening the top bar does not require a third-party request", () => {
  const bar = open();
  const github = [...bar.container.querySelectorAll("a")].at(-1);

  assert.equal(github?.textContent?.trim(), "GitHub");
  assert.equal(github?.getAttribute("href"), "https://github.com/viniciusnevescosta/normfix");
});

test("the documentation link follows the reader's language", () => {
  const bar = open({ docsHref: "/docs/fr/" });
  const docs = [...bar.container.querySelectorAll("a")][0];

  assert.equal(docs?.getAttribute("href"), "/docs/fr/");
});
