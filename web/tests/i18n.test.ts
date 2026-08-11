import assert from "node:assert/strict";
import test from "node:test";

import { SUPPORTED_LOCALES, translationCatalogueProblems, translatePlural } from "../src/i18n";

test("every advertised locale translates the complete browser catalogue", () => {
  assert.deepEqual(translationCatalogueProblems(), []);
});

test("a count agrees with the noun beside it in every language", () => {
  // "1 arquivos adicionados" is the failure this exists to prevent: a
  // placeholder alone cannot make a noun agree with its number.
  for (const locale of SUPPORTED_LOCALES) {
    const one = translatePlural(locale, "imported", 1);
    const many = translatePlural(locale, "imported", 5);

    assert.notEqual(one, many, `${locale} uses one wording for 1 and for 5`);
    assert.ok(!one.includes("{count}"), `${locale} left a placeholder unfilled`);
    assert.ok(many.includes("5"), `${locale} dropped the count`);
  }
});

test("an unpublished plural category falls back instead of failing", () => {
  // Arabic has categories English never uses. A locale added later must not
  // need a new call site to be usable.
  assert.equal(translatePlural("en", "skipped", 0), translatePlural("en", "skipped", 0));
  assert.ok(translatePlural("en", "skipped", 12).includes("12"));
});
