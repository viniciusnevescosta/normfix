<script lang="ts">
// The one question this page asks before doing something it cannot undo.
//
// It was wired through the dialog's `close` event and its return value, and
// that pair never fired here: confirming quietly did nothing, which is worse
// than having no confirmation at all. The buttons are wired directly now, so
// there is one path from the click to the deletion.
import { t as translate } from "../i18n-state.svelte";

interface Props {
  /** What is being deleted, or `null` when nothing is being asked. */
  request: { text: string } | null;
  onConfirm: () => void;
  onCancel: () => void;
}

const { request, onConfirm, onCancel }: Props = $props();

let dialog = $state<HTMLDialogElement | null>(null);

// A native modal is opened and closed by method, not by attribute, so the
// element is followed rather than described.
$effect(() => {
  if (!dialog) return;
  if (request && !dialog.open) dialog.showModal();
  if (!request && dialog.open) dialog.close();
});
</script>

<dialog
  bind:this={dialog}
  class="border-border bg-panel rounded border p-4"
  oncancel={(event) => {
    event.preventDefault();
    onCancel();
  }}
>
  {#if request}
    <h2 class="font-semibold">{translate("deleteEntry")}</h2>
    <p class="mt-2 text-sm">{request.text}</p>
    <div class="mt-4 flex justify-end gap-2">
      <button class="border-border rounded border px-3 py-1 text-sm" type="button" onclick={onCancel}>
        {translate("cancel")}
      </button>
      <button class="border-error text-error rounded border px-3 py-1 text-sm" type="button" onclick={onConfirm}>
        {translate("deleteEntry")}
      </button>
    </div>
  {/if}
</dialog>
