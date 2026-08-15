<script lang="ts">
// What covers the editor when there is nothing to edit.
//
// Two states share this: a file normfix cannot format, and a project with no
// files at all. Both used to be a `hidden` attribute the page set by hand,
// and both were caught on screen anyway — a `display` rule outranks the
// browser's own `[hidden]`, silently, while a script asking `element.hidden`
// is told what it wants to hear. There is no attribute to disagree with now:
// the notice exists when there is something to say and does not when there
// is not.
interface Props {
  /** What to say, or `null` to leave the editor alone. */
  notice: { title: string; detail: string } | null;
}

const { notice }: Props = $props();
</script>

{#if notice}
  <div
    class="bg-panel absolute inset-0 z-10 flex flex-col items-center justify-center gap-1 text-center"
    role="status"
  >
    <strong>{notice.title}</strong>
    <span class="text-muted text-sm">{notice.detail}</span>
  </div>
{/if}
