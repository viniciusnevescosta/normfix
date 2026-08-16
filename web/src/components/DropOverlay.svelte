<script lang="ts">
// What the page says while a folder is being dragged over it.
//
// The overlay was another `hidden` attribute the page toggled by hand, in a
// codebase where that attribute has already been overridden by a `display`
// rule twice. It exists while a drag is over the page and does not
// otherwise, so there is no attribute to get wrong.
//
// It says what will be taken before anything is dropped, because a drop that
// silently keeps four files out of seven is one the reader has to reverse
// engineer afterwards.
import { t as translate } from "../i18n-state.svelte";

interface Props {
  active: boolean;
}

const { active }: Props = $props();
</script>

{#if active}
  <div
    class="bg-scrim fixed inset-0 z-40 flex items-center justify-center"
    aria-hidden="true"
  >
    <div class="border-border bg-panel flex flex-col gap-1 rounded border p-6 text-center">
      <strong>{translate("dropHere")}</strong>
      <span class="text-muted text-sm">{translate("dropAccepted")}</span>
    </div>
  </div>
{/if}
