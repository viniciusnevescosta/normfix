// The reader's language, as reactive state.
//
// Passing a `translate` function into a component looks fine and is not: a
// function is not state, so changing the language changed nothing a component
// was watching, and every migrated panel kept the words it had rendered with.
// The language selector is the one control on this page whose whole job is to
// change every other word on it.
//
// Reading the locale here makes any component that translates depend on it,
// which is exactly the dependency that has to exist.
import {
  detectLocale,
  type Locale,
  translate as lookup,
  type MessageKey,
  translatePlural,
} from "./i18n";

const state = $state({ locale: detectLocale() });

/** Switches the page's language, and everything that reads it with it. */
export function setLocale(locale: Locale): void {
  state.locale = locale;
}

/** One translated message, in whatever language the page is in now. */
export function t(key: string, values?: Record<string, string | number>): string {
  return lookup(state.locale, key as MessageKey, values ?? {});
}

/** A message whose noun has to agree with the number beside it. */
export function tPlural(base: string, count: number): string {
  return translatePlural(state.locale, base, count);
}
