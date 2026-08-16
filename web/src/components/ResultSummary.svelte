<script lang="ts">
// What a run produced, across the project and for the file being looked at.
//
// The counts, the file picker, the actions and the tabs were four things the
// page disabled and re-enabled by hand after every run, every apply and every
// edit. Each is derived here from the one result being shown, so a button
// that would do nothing cannot be left clickable.
interface Summary {
  files: number;
  changed: number;
  fixes: number;
  diagnostics: number;
  failed: number;
}

import { t as translate } from "../i18n-state.svelte";

interface Props {
  summary: Summary;
  paths: readonly string[];
  selected: string;
  /** Whether the shown result can be applied, copied or downloaded. */
  usable: boolean;
  /** How many results across the project are worth applying at once. */
  applicable: number;
  diagnosticCount: number;
  view: "formatted" | "diagnostics" | "diff";
  copyLabel: string;
  onSelect: (path: string) => void;
  onView: (view: "formatted" | "diagnostics" | "diff") => void;
  onApply: () => void;
  onApplyAll: () => void;
  onCopy: () => void;
  onDownload: () => void;
  onDownloadAll: () => void;
}

const {
  summary,
  paths,
  selected,
  usable,
  applicable,
  diagnosticCount,
  view,
  copyLabel,
  onSelect,
  onView,
  onApply,
  onApplyAll,
  onCopy,
  onDownload,
  onDownloadAll,
}: Props = $props();

const counts = $derived([
  [summary.files, "filesSummary"],
  [summary.changed, "changedSummary"],
  [summary.fixes, "fixesSummary"],
  [summary.diagnostics, "diagnosticsSummary"],
  [summary.failed, "failedSummary"],
] as const);

const tabs = [
  ["formatted", "formatted"],
  ["diagnostics", "diagnostics"],
  ["diff", "diff"],
] as const;
</script>

<div class="flex flex-wrap gap-3 text-sm" aria-label={translate("runSummary")}>
  {#each counts as [value, label] (label)}
    <span class="flex items-baseline gap-1">
      <strong>{value}</strong><span class="text-muted">{translate(label)}</span>
    </span>
  {/each}
</div>

<div class="mt-3 flex flex-wrap items-end gap-3">
  <label class="flex flex-col gap-1 text-sm">
    <span class="text-faint text-xs uppercase">{translate("resultFile")}</span>
    <select
      class="border-border bg-surface-sunken rounded border px-2 py-1 font-mono text-sm"
      value={selected}
      onchange={(event) => onSelect((event.currentTarget as HTMLSelectElement).value)}
    >
      {#each paths as path (path)}
        <option value={path}>{path}</option>
      {/each}
    </select>
  </label>

  <div class="flex flex-wrap gap-2">
    <!-- Disabled is derived, not remembered: a result that cannot be applied
         cannot leave a button that says it can. -->
    <button
      class="border-border rounded border px-3 py-1 text-sm disabled:opacity-50"
      type="button"
      disabled={applicable === 0}
      onclick={onApplyAll}
    >
      {translate("fixAll")}
    </button>
    <button
      class="border-border rounded border px-3 py-1 text-sm disabled:opacity-50"
      type="button"
      disabled={!usable}
      onclick={onApply}
    >
      {translate("fixCurrent")}
    </button>
    <button
      class="border-border rounded border px-3 py-1 text-sm disabled:opacity-50"
      type="button"
      disabled={!usable}
      onclick={onCopy}
    >
      {copyLabel}
    </button>
    <button
      class="border-border rounded border px-3 py-1 text-sm disabled:opacity-50"
      type="button"
      disabled={!usable}
      onclick={onDownload}
    >
      {translate("downloadFile")}
    </button>
    <button
      class="border-border rounded border px-3 py-1 text-sm"
      type="button"
      onclick={onDownloadAll}
    >
      {translate("downloadAll")}
    </button>
  </div>
</div>

<div class="border-border mt-3 flex gap-1 border-b" role="tablist" aria-label={translate("resultViews")}>
  {#each tabs as [name, label] (name)}
    <button
      class="px-3 py-1 text-sm"
      class:border-accent={view === name}
      class:border-b-2={view === name}
      type="button"
      role="tab"
      id="{name}-tab"
      aria-selected={view === name}
      aria-controls="{name}-view"
      onclick={() => onView(name)}
    >
      {translate(label)}
      {#if name === "diagnostics"}<span class="text-faint ml-1">{diagnosticCount}</span>{/if}
    </button>
  {/each}
</div>
