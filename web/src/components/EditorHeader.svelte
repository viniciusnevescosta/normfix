<script lang="ts">
// Which file is open and how big it is.
//
// Two lines of text the page wrote by hand from four places — selecting a
// file, opening one it cannot format, emptying the project, and every
// keystroke. Written from one object now, so an empty title and a stale byte
// count cannot be on screen together.
interface Props {
  /** The open file's path, or `null` when nothing is open. */
  path: string | null;
  lines: number;
  bytes: number;
  measure: (lines: number, bytes: number) => string;
  label: string;
}

const { path, lines, bytes, measure, label }: Props = $props();
</script>

<div class="flex items-baseline justify-between gap-3 p-2">
  <div>
    <span class="text-faint text-xs uppercase">{label}</span>
    <strong class="ml-2 font-mono text-sm">{path ?? ""}</strong>
  </div>
  <span class="text-muted text-xs">{path === null ? "" : measure(lines, bytes)}</span>
</div>
