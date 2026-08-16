import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "vitest";

const page = await readFile(new URL("../index.html", import.meta.url), "utf8");
const bootstrap = await readFile(new URL("../src/main.ts", import.meta.url), "utf8");

test("every element the page asks for by id is in the markup", async () => {
  // main.ts finds its mount points and its remaining controls by id, and
  // nothing else checks that they exist: TypeScript sees the TypeScript, Biome
  // sees the script blocks, and neither has ever opened index.html. Cutting one
  // block out of the markup took a container with it, and the page ran for five
  // commits as a single stacked column before a screenshot found it.
  const required = [...bootstrap.matchAll(/requiredElement<[^>]*>\("#([\w-]+)"\)/g)].map(
    (match) => match[1],
  );

  assert.ok(required.length > 10, "the bootstrap still finds its elements by id");
  const missing = required.filter((id) => !page.includes(`id="${id}"`));
  assert.deepEqual(missing, [], "these ids are asked for but not in the page");
});

test("the workbench keeps the shape the stylesheet lays out", async () => {
  // These are not mount points, so nothing would miss them: they are the grid
  // the panels sit in. Losing one does not break a single behaviour, and turns
  // the page into one column.
  for (const container of [
    'class="workbench"',
    'class="files-panel"',
    'class="editor-panel"',
    'class="results"',
    'class="editor-surface"',
  ]) {
    assert.ok(page.includes(container), `${container} is part of the layout`);
  }
});

test("a mount point holds nothing of its own", async () => {
  // A component replaces whatever is inside its target, so markup left there is
  // markup that flashes on load and then vanishes — or worse, is what the
  // reader keeps when the component fails to mount.
  const mounts = [...bootstrap.matchAll(/target: elements\.(\w+)/g)].map((match) => match[1]);
  assert.ok(mounts.length > 5, "components are mounted into the page");

  for (const name of new Set(mounts)) {
    const selector = bootstrap.match(
      new RegExp(`${name}: requiredElement<[^>]*>\\("#([\\w-]+)"\\)`),
    )?.[1];
    if (!selector) continue;
    const element = page.match(new RegExp(`<div id="${selector}"[^>]*>([\\s\\S]*?)</div>`))?.[1];
    assert.equal(element?.trim() ?? "", "", `#${selector} is a mount point and should be empty`);
  }
});
