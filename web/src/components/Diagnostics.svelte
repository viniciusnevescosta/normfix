<script lang="ts">
// What a run found in one file: why nothing was formatted when that is the
// answer, then the findings, then what was fixed, then how much room each
// function has left.
//
// The order is the point. A file that will not parse has the one finding
// that matters most — where the parser lost its way — and the imperative
// version returned before reaching it, leaving a reader with a sentence
// about ERROR and MISSING bytes and no line to look at.
interface Location {
  line: number;
  column: number;
}

interface Diagnostic {
  rule_id: string;
  severity: string;
  message: string;
  location: Location | null;
  help: string | null;
  source: string;
}

interface Fix {
  rule_id: string;
  description: string;
}

interface Budget {
  function: string;
  line: number;
  lines: number;
  line_limit: number;
  variables: number;
  variable_limit: number;
  parameters: number;
  parameter_limit: number;
}

interface Props {
  diagnostics: readonly Diagnostic[];
  fixes: readonly Fix[];
  budget: readonly Budget[];
  /** The reason nothing was written, when there is one. */
  error: string | null;
  stable: boolean;
  translate: (key: string, values?: Record<string, string | number>) => string;
}

const { diagnostics, fixes, budget, error, stable, translate }: Props = $props();

// A file the parser could not read carries the reason as an error; an
// unstable run carries none. `stable` is false for both, so reading it to
// tell them apart sent every unreadable file the fixed-point message.
const unwritten = $derived(Boolean(error) || !stable);
</script>

{#if unwritten}
  <div class="border-border mb-3 rounded border p-3">
    <strong class="block">{translate("fileUnchanged")}</strong>
    <span class="text-muted text-sm">
      {error ? translate("unparsableFile") : translate("unstableFormatter")}
    </span>
  </div>
{/if}

{#if diagnostics.length === 0 && !unwritten}
  <div class="border-border rounded border p-3">
    <strong class="block">{translate("noDiagnostics")}</strong>
    <span class="text-muted text-sm">{translate("cliCoverage")}</span>
  </div>
{/if}

{#each diagnostics as diagnostic (diagnostic.rule_id + (diagnostic.location?.line ?? 0) + diagnostic.message)}
  <article class="border-border mb-2 rounded border p-2" data-severity={diagnostic.severity}>
    <header class="flex flex-wrap items-baseline gap-2 font-mono text-xs">
      <span
        class:text-error={diagnostic.severity === "error"}
        class:text-warning={diagnostic.severity === "warning"}
        class:text-info={diagnostic.severity === "info"}
      >
        {diagnostic.severity}
      </span>
      <span>{diagnostic.rule_id}</span>
      {#if diagnostic.location}
        <span class="text-faint">L{diagnostic.location.line}:C{diagnostic.location.column}</span>
      {/if}
    </header>
    <p class="mt-1 text-sm">{diagnostic.message}</p>
    <p class="text-muted mt-1 text-xs">
      {diagnostic.help ? `${translate("next")}: ${diagnostic.help}` : diagnostic.source}
    </p>
  </article>
{/each}

{#if fixes.length > 0}
  <section class="mt-3">
    <h3 class="text-sm font-semibold">
      {translate("fixesApplied", { count: fixes.length })}
    </h3>
    <ul class="mt-1 flex flex-col gap-1 text-sm">
      {#each fixes as fix, index (fix.rule_id + index)}
        <li><code class="text-faint mr-2 text-xs">{fix.rule_id}</code>{fix.description}</li>
      {/each}
    </ul>
  </section>
{/if}

{#if budget.length > 0}
  <section class="mt-3 overflow-x-auto">
    <h3 class="text-sm font-semibold">{translate("functionBudget")}</h3>
    <table class="mt-1 w-full text-left font-mono text-xs">
      <thead>
        <tr>
          {#each [translate("function"), translate("line"), translate("bodyLines"), translate("variables"), translate("parameters")] as label (label)}
            <th scope="col" class="text-faint pr-3 font-normal">{label}</th>
          {/each}
        </tr>
      </thead>
      <tbody>
        {#each budget as entry (entry.function + entry.line)}
          <tr>
            <td class="pr-3">{entry.function}()</td>
            <td class="pr-3">{entry.line}</td>
            <!-- Over the limit is marked rather than left to the reader to
                 compare two numbers in every row. -->
            <td class="pr-3" class:text-error={entry.lines > entry.line_limit}>
              {entry.lines}/{entry.line_limit}
            </td>
            <td class="pr-3" class:text-error={entry.variables > entry.variable_limit}>
              {entry.variables}/{entry.variable_limit}
            </td>
            <td class="pr-3" class:text-error={entry.parameters > entry.parameter_limit}>
              {entry.parameters}/{entry.parameter_limit}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>
{/if}
