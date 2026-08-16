<script lang="ts">
// The formatted output and the diff, which are text and nothing else.
//
// Both were `textContent =` assignments into elements found by id, and the
// copy path reached back for one of them to select its contents. The element
// is handed out here instead, so the one caller that needs it says so rather
// than looking it up.
interface Props {
  text: string;
  /** Receives the element holding the text, for selecting it to copy. */
  bind?: (element: HTMLElement) => void;
}

const { text, bind }: Props = $props();

let code = $state<HTMLElement | null>(null);

$effect(() => {
  if (code && bind) bind(code);
});
</script>

<pre class="overflow-x-auto p-3 font-mono text-xs"><code bind:this={code}>{text}</code></pre>
