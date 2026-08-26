<script lang="ts">
// The 42 identity the header is written with.
//
// The panel used to offer "remember on this device" and both buttons whether
// or not anything was remembered, so the one thing a reader wants to know —
// is my email on this machine — was the one thing it would not say. What is
// shown is derived from whether something is stored, which means the two can
// no longer disagree.
import { t as translate } from "../i18n-state.svelte";

interface Props {
  email: string;
  stored: boolean;
  status: string;
  invalid: boolean;
  onSave: (email: string, remember: boolean) => void;
  onForget: () => void;
}

const { email, stored, status, invalid, onSave, onForget }: Props = $props();

let typed = $state("");
let remember = $state(false);

// The field follows the project rather than copying it once: the project
// rewrites the address after a save, to its canonical form, and empties it
// after forgetting. Seeding from the prop instead would capture whatever it
// held on the first render and then quietly disagree.
$effect(() => {
  typed = email;
});
</script>

<form
  class="flex flex-col gap-2 p-3"
  aria-labelledby="identity-title"
  onsubmit={(event) => {
    event.preventDefault();
    if (!stored) onSave(typed, remember);
  }}
>
  <strong id="identity-title">{translate("identity")}</strong>
  <label class="text-faint text-xs uppercase" for="identity-email">{translate("email")}</label>
  <input
    id="identity-email"
    class="border-border bg-surface-sunken rounded border px-2 py-1 font-mono text-sm"
    type="email"
    inputmode="email"
    autocomplete="off"
    autocapitalize="none"
    spellcheck="false"
    maxlength="254"
    enterkeyhint="done"
    placeholder={translate("emailPlaceholder")}
    aria-invalid={invalid ? "true" : undefined}
    aria-describedby="identity-status identity-privacy"
    readonly={stored}
    bind:value={typed}
  />

  <!-- With an identity stored the box has nothing left to ask: it would offer
       a choice already made. -->
  {#if !stored}
    <label class="flex items-center gap-2 text-sm">
      <input type="checkbox" bind:checked={remember} />
      <span>{translate("rememberIdentity")}</span>
    </label>
  {/if}

  <div class="flex items-center gap-2">
    {#if stored}
      <button class="text-accent text-sm underline" type="button" onclick={onForget}>
        {translate("forgetIdentity")}
      </button>
    {:else}
      <button
        class="border-border hover:bg-surface-selected rounded border px-3 py-1 text-sm"
        type="submit"
      >
        {translate("saveIdentity")}
      </button>
    {/if}
  </div>

  <p id="identity-status" class="text-muted min-h-4 text-xs" role="status">{status}</p>
  <p id="identity-privacy" class="text-faint text-xs">{translate("identityPrivacy")}</p>
</form>
