<script lang="ts">
// The two badges in the top bar: whether the formatter is ready, and what
// offline support is doing.
//
// The offline one says nothing while it is simply working, because that is
// the normal state of an installed playground and a badge that is always
// talking is one nobody reads. It speaks for the two events that change what
// the reader can do: an update waiting to be taken, and being offline
// without support.
import { t as translate } from "../i18n-state.svelte";

interface Props {
  runtime: "loading" | "ready" | "error";
  runtimeLabel: string;
  offline: string;
  online: boolean;
  onUpdate: () => void;
}

const { runtime, runtimeLabel, offline, online, onUpdate }: Props = $props();

const updateReady = $derived(offline === "update-ready");
// It speaks for the two events that change what the reader can do, and is
// absent otherwise: a badge that is always talking is one nobody reads.
const speaks = $derived(updateReady || (offline === "ready" && !online));
</script>

<div class="flex items-center gap-2 text-xs" data-state={runtime} role="status">
  <span
    class="h-2 w-2 rounded-full"
    class:bg-faint={runtime === "loading"}
    class:bg-accent={runtime === "ready"}
    class:bg-error={runtime === "error"}
    aria-hidden="true"
  ></span>
  <span>{runtimeLabel}</span>
</div>

{#if speaks}
  <div
  class="border-border text-muted flex items-center gap-2 border px-2 py-1 text-xs"
  data-state={offline}
  data-online={online}
  aria-label={translate("offlineAvailability")}
>
  <span class="bg-faint h-2 w-2 rounded-full" aria-hidden="true"></span>
  <span role="status">
    {updateReady ? translate("offlineUpdate") : translate("offlineActive")}
  </span>
  {#if updateReady}
    <button class="text-accent underline" type="button" onclick={onUpdate}>
      {translate("offlineUpdateAction")}
    </button>
  {/if}
  </div>
{/if}
