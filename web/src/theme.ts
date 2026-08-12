// Appearance: follow the system, or override it.
//
// The choice is remembered on the device, like the language. What it is *not*
// is a second way to say something about the code: it changes colors and
// nothing else, so switching it never triggers a run, a fetch, or a reload.

export const THEME_PREFERENCES = ["system", "light", "dark"] as const;

/** What the reader asked for. `system` defers to the operating system. */
export type ThemePreference = (typeof THEME_PREFERENCES)[number];

/** What is actually painted. */
export type Appearance = "light" | "dark";

const STORAGE_KEY = "normfix.theme.v1";

export function isThemePreference(value: unknown): value is ThemePreference {
  return (THEME_PREFERENCES as readonly unknown[]).includes(value);
}

/**
 * Resolves a preference against the system setting.
 *
 * Dark is the fallback rather than light: it is the design the playground was
 * built in, so a browser that reports nothing gets the tested appearance
 * instead of a palette chosen by accident.
 */
export function resolveAppearance(
  preference: ThemePreference,
  systemPrefersLight: boolean,
): Appearance {
  if (preference === "light") return "light";
  if (preference === "dark") return "dark";
  return systemPrefersLight ? "light" : "dark";
}

export function readStoredThemePreference(): ThemePreference {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (isThemePreference(stored)) return stored;
  } catch {
    // A blocked storage API must not stop the playground from starting.
  }
  return "system";
}

export function storeThemePreference(preference: ThemePreference): void {
  try {
    localStorage.setItem(STORAGE_KEY, preference);
  } catch {
    // The choice still applies to this page.
  }
}

/**
 * Paints a preference.
 *
 * `system` deliberately removes the attribute rather than writing a resolved
 * value, so the stylesheet's own `prefers-color-scheme` rule stays in charge.
 * That is what lets a reader who never chose anything get the right colors on
 * the first frame, before this script has run at all.
 */
export function applyThemePreference(preference: ThemePreference): Appearance {
  const systemPrefersLight = matchMedia("(prefers-color-scheme: light)").matches;
  const appearance = resolveAppearance(preference, systemPrefersLight);
  const root = document.documentElement;
  if (preference === "system") root.removeAttribute("data-theme");
  else root.dataset.theme = preference;
  document
    .querySelector<HTMLMetaElement>('meta[name="theme-color"]')
    ?.setAttribute("content", appearance === "light" ? "#f7f8f7" : "#111311");
  return appearance;
}

/** Calls back when the system setting changes, which only matters under `system`. */
export function watchSystemAppearance(onChange: () => void): void {
  matchMedia("(prefers-color-scheme: light)").addEventListener("change", onChange);
}
