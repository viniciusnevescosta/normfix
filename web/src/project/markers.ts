// Turning findings into the marks an editor draws under the code.
//
// The diagnostics carry a position, not a span, so a mark runs from that column
// to the end of its line rather than guessing at a width. A finding with no
// position at all belongs to the file rather than to a line, and drawing it
// somewhere would put a squiggle under code that has nothing to do with it.

/** One finding to underline, in one-based editor coordinates. */
export interface EditorMarker {
  severity: "error" | "warning" | "info";
  message: string;
  ruleId: string;
  line: number;
  column: number;
}

interface MarkableDiagnostic {
  rule_id: string;
  severity: "error" | "warning" | "info";
  message: string;
  location: { line: number; column: number } | null;
}

export function markersFor(diagnostics: readonly MarkableDiagnostic[]): EditorMarker[] {
  return diagnostics
    .filter((diagnostic) => diagnostic.location !== null)
    .map((diagnostic) => ({
      severity: diagnostic.severity,
      message: diagnostic.message,
      ruleId: diagnostic.rule_id,
      // A backend that reports line or column zero would place a mark outside
      // the document, which Monaco resolves by clamping somewhere unhelpful.
      line: Math.max(diagnostic.location?.line ?? 1, 1),
      column: Math.max(diagnostic.location?.column ?? 1, 1),
    }));
}
