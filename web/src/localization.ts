import type { PlaygroundElements } from "./dom";
import { detectLocale, type Locale, type MessageKey, SUPPORTED_LOCALES } from "./i18n";

const LOCALE_STORAGE_KEY = "normfix.locale.v1";

export function readInitialLocale(): Locale {
  const routeLocale = window.location.pathname.split("/").filter(Boolean)[0];
  if (routeLocale && SUPPORTED_LOCALES.includes(routeLocale as Locale)) {
    return routeLocale as Locale;
  }
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (SUPPORTED_LOCALES.includes(stored as Locale)) return stored as Locale;
  } catch {
    // A blocked storage API should not prevent the playground from starting.
  }
  return detectLocale();
}

export function storeLocale(locale: Locale): void {
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // The selected language still applies for the current page.
  }
}

/** Translates the static shell and updates route-aware metadata in one pass. */
export function localizeDocument(
  locale: Locale,
  elements: PlaygroundElements,
  translate: (key: MessageKey) => string,
): { docsHref: string } {
  document.documentElement.lang = locale;
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    const key = element.dataset.i18n as MessageKey | undefined;
    if (key) element.textContent = translate(key);
  }
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
    const key = element.dataset.i18nTitle as MessageKey | undefined;
    if (key) element.title = translate(key);
  }
  for (const element of document.querySelectorAll<HTMLInputElement>("[data-i18n-placeholder]")) {
    const key = element.dataset.i18nPlaceholder as MessageKey | undefined;
    if (key) element.placeholder = translate(key);
  }
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n-aria]")) {
    const key = element.dataset.i18nAria as MessageKey | undefined;
    if (key) element.setAttribute("aria-label", translate(key));
  }

  const route = locale === "en" ? "/" : `/${locale}/`;
  if (window.location.pathname !== route) {
    window.history.replaceState(
      null,
      "",
      `${route}${window.location.search}${window.location.hash}`,
    );
  }
  const canonical = `https://normfix.vercel.app${route}`;
  const title = translate("seoTitle");
  const description = translate("seoDescription");
  document.title = title;
  elements.metaDescription.content = description;
  elements.ogTitle.content = title;
  elements.ogDescription.content = description;
  elements.twitterTitle.content = title;
  elements.twitterDescription.content = description;
  elements.ogUrl.content = canonical;

  const ogLocale =
    locale === "pt" ? "pt_BR" : locale === "es" ? "es_ES" : locale === "fr" ? "fr_FR" : "en_US";
  elements.ogLocale.content = ogLocale;
  ["en_US", "pt_BR", "es_ES", "fr_FR"]
    .filter((candidate) => candidate !== ogLocale)
    .forEach((candidate, index) => {
      const meta = elements.ogAlternates[index];
      if (meta) meta.content = candidate;
    });
  elements.canonical.href = canonical;
  elements.manifest.href = `${route}site.webmanifest`;
  elements.brand.href = route;
  return { docsHref: locale === "en" ? "/docs/" : `/docs/${locale}/` };
}
