<script lang="ts">
// The controls that sit above everything: language, appearance, and the two
// links out.
//
// Language is the one control on this page whose whole job is to change every
// other word on it, and the star count is decoration that must never look like
// a number it could not fetch.
import { t as translate } from "../i18n-state.svelte";

interface Props {
  locale: string;
  theme: string;
  /** The star count, or `null` when GitHub could not be reached. */
  stars: number | null;
  /** Formats the count for the reader's locale. */
  format: (stars: number) => string;
  docsHref: string;
  onLocale: (locale: string) => void;
  onTheme: (theme: string) => void;
}

const { locale, theme, stars, format, docsHref, onLocale, onTheme }: Props = $props();

const locales = ["en", "pt", "es", "fr"] as const;
const themes = ["system", "light", "dark"] as const;
</script>

<label class="flex items-center gap-1">
  <span class="sr-only">{translate("language")}</span>
  <select
    class="border-border bg-surface-sunken rounded border px-2 py-1 text-xs"
    aria-label={translate("language")}
    value={locale}
    onchange={(event) => onLocale((event.currentTarget as HTMLSelectElement).value)}
  >
    {#each locales as code (code)}
      <option value={code}>{code.toUpperCase()}</option>
    {/each}
  </select>
</label>

<label class="flex items-center gap-1">
  <span class="sr-only">{translate("appearance")}</span>
  <select
    class="border-border bg-surface-sunken rounded border px-2 py-1 text-xs"
    aria-label={translate("appearance")}
    value={theme}
    onchange={(event) => onTheme((event.currentTarget as HTMLSelectElement).value)}
  >
    {#each themes as name (name)}
      <option value={name}>
        {translate(`appearance${name[0]?.toUpperCase()}${name.slice(1)}`)}
      </option>
    {/each}
  </select>
</label>

<a
  class="border-border-strong hover:border-accent hover:text-accent border px-2.5 py-0.5 text-xs no-underline"
  href={docsHref}
>{translate("docs")}</a>

<a
  class="border-border-strong hover:border-accent hover:text-accent flex items-center gap-1 border px-2.5 py-0.5 text-xs no-underline"
  href="https://github.com/viniciusnevescosta/normfix"
  target="_blank"
  rel="noopener noreferrer"
  title={stars === null ? translate("githubFallback") : undefined}
>
  <span>{translate("github")}</span>
  <span aria-hidden="true">★</span>
  <!-- A count that could not be fetched says zero and explains itself in the
       title, rather than showing a number nobody measured. -->
  <span aria-live="polite">{stars === null ? "0" : format(stars)}</span>
</a>
