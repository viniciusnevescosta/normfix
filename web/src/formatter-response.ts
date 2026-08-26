import { MAX_FILE_BYTES, MAX_FILES, MAX_PROJECT_BYTES } from "./project/files";

export type Severity = "error" | "warning" | "info";

export interface BrowserLocation {
  line: number;
  column: number;
}

export interface BrowserFix {
  rule_id: string;
  description: string;
  line: number | null;
  applicability: string;
}

export interface BrowserDiagnostic {
  rule_id: string;
  severity: Severity;
  message: string;
  location: BrowserLocation | null;
  help: string | null;
  notes: string[];
  source: string;
}

export interface BrowserBudget {
  function: string;
  line: number;
  lines: number;
  line_limit: number;
  variables: number;
  variable_limit: number;
  parameters: number;
  parameter_limit: number;
}

export interface BrowserFileResult {
  path: string;
  formatted: string;
  changed: boolean;
  stable: boolean;
  fixes: BrowserFix[];
  diagnostics: BrowserDiagnostic[];
  budget: BrowserBudget[];
  diff: string;
  error: string | null;
}

export interface BrowserSummary {
  files: number;
  changed: number;
  fixes: number;
  diagnostics: number;
  failed: number;
}

export interface PlaygroundResponse {
  schema_version: 1;
  files: BrowserFileResult[];
  summary: BrowserSummary;
}

export class FormatterResponseError extends Error {
  constructor(
    readonly code: "schema" | "path",
    readonly path: string | null = null,
  ) {
    super(code);
    this.name = "FormatterResponseError";
  }
}

const ENCODER = new TextEncoder();
const MAX_ITEMS_PER_FILE = 4096;
const MAX_TEXT_FIELD_BYTES = 64 * 1024;
const MAX_FORMATTED_BYTES = MAX_FILE_BYTES * 2 + MAX_TEXT_FIELD_BYTES;
const MAX_DIFF_BYTES = MAX_FILE_BYTES * 8 + MAX_TEXT_FIELD_BYTES;
const MAX_TOTAL_FORMATTED_BYTES = MAX_PROJECT_BYTES * 2 + MAX_FILES * MAX_TEXT_FIELD_BYTES;
const MAX_TOTAL_DIFF_BYTES = MAX_PROJECT_BYTES * 8 + MAX_FILES * MAX_TEXT_FIELD_BYTES;
const MAX_RESPONSE_BYTES = MAX_TOTAL_FORMATTED_BYTES + MAX_TOTAL_DIFF_BYTES + MAX_PROJECT_BYTES * 2;

function record(value: unknown): Record<string, unknown> {
  if (typeof value !== "object" || value === null || Array.isArray(value)) {
    throw new FormatterResponseError("schema");
  }
  return value as Record<string, unknown>;
}

function text(value: unknown, maxBytes = MAX_TEXT_FIELD_BYTES): string {
  if (typeof value !== "string" || ENCODER.encode(value).length > maxBytes) {
    throw new FormatterResponseError("schema");
  }
  return value;
}

function integer(value: unknown, minimum = 0): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < minimum) {
    throw new FormatterResponseError("schema");
  }
  return value;
}

function nullableText(value: unknown): string | null {
  return value === null ? null : text(value);
}

function parseFix(value: unknown): BrowserFix {
  const item = record(value);
  return {
    rule_id: text(item.rule_id),
    description: text(item.description),
    line: item.line === null ? null : integer(item.line),
    applicability: text(item.applicability),
  };
}

function parseDiagnostic(value: unknown): BrowserDiagnostic {
  const item = record(value);
  const severity = item.severity;
  if (severity !== "error" && severity !== "warning" && severity !== "info") {
    throw new FormatterResponseError("schema");
  }
  const location =
    item.location === null
      ? null
      : (() => {
          const point = record(item.location);
          return { line: integer(point.line, 1), column: integer(point.column, 1) };
        })();
  if (!Array.isArray(item.notes) || item.notes.length > MAX_ITEMS_PER_FILE) {
    throw new FormatterResponseError("schema");
  }
  return {
    rule_id: text(item.rule_id),
    severity,
    message: text(item.message),
    location,
    help: nullableText(item.help),
    notes: item.notes.map((note) => text(note)),
    source: text(item.source),
  };
}

function parseBudget(value: unknown): BrowserBudget {
  const item = record(value);
  return {
    function: text(item.function),
    line: integer(item.line, 1),
    lines: integer(item.lines),
    line_limit: integer(item.line_limit),
    variables: integer(item.variables),
    variable_limit: integer(item.variable_limit),
    parameters: integer(item.parameters),
    parameter_limit: integer(item.parameter_limit),
  };
}

function boundedArray<T>(value: unknown, parse: (entry: unknown) => T): T[] {
  if (!Array.isArray(value) || value.length > MAX_ITEMS_PER_FILE) {
    throw new FormatterResponseError("schema");
  }
  return value.map(parse);
}

function parseFile(value: unknown, inputs: ReadonlyMap<string, string>): BrowserFileResult {
  const item = record(value);
  const path = text(item.path);
  if (!inputs.has(path)) throw new FormatterResponseError("path", path);
  if (typeof item.changed !== "boolean" || typeof item.stable !== "boolean") {
    throw new FormatterResponseError("schema");
  }
  return {
    path,
    formatted: text(item.formatted, MAX_FORMATTED_BYTES),
    changed: item.changed,
    stable: item.stable,
    fixes: boundedArray(item.fixes, parseFix),
    diagnostics: boundedArray(item.diagnostics, parseDiagnostic),
    budget: boundedArray(item.budget, parseBudget),
    diff: text(item.diff, MAX_DIFF_BYTES),
    error: nullableText(item.error),
  };
}

/** Parses the WebAssembly boundary and refuses partial, duplicate, or oversized output. */
export function parseFormatterResponse(
  payload: string,
  inputs: ReadonlyMap<string, string>,
): PlaygroundResponse {
  // Refuse a runaway formatter result before JSON.parse duplicates it into an
  // object graph. The WASM is bundled and trusted, but an accidental diff
  // explosion must not freeze or exhaust a browser tab.
  if (payload.length > MAX_RESPONSE_BYTES || ENCODER.encode(payload).length > MAX_RESPONSE_BYTES) {
    throw new FormatterResponseError("schema");
  }
  let decoded: unknown;
  try {
    decoded = JSON.parse(payload);
  } catch {
    throw new FormatterResponseError("schema");
  }
  const response = record(decoded);
  if (response.schema_version !== 1 || !Array.isArray(response.files)) {
    throw new FormatterResponseError("schema");
  }
  if (response.files.length !== inputs.size || response.files.length > MAX_FILES) {
    throw new FormatterResponseError("schema");
  }
  const files = response.files.map((file) => parseFile(file, inputs));
  const paths = new Set(files.map((file) => file.path));
  if (paths.size !== files.length) throw new FormatterResponseError("schema");
  for (const path of inputs.keys()) {
    if (!paths.has(path)) throw new FormatterResponseError("path", path);
  }
  const formattedBytes = files.reduce(
    (total, file) => total + ENCODER.encode(file.formatted).length,
    0,
  );
  if (formattedBytes > MAX_TOTAL_FORMATTED_BYTES) throw new FormatterResponseError("schema");
  const diffBytes = files.reduce((total, file) => total + ENCODER.encode(file.diff).length, 0);
  if (diffBytes > MAX_TOTAL_DIFF_BYTES) throw new FormatterResponseError("schema");

  const summary = {
    files: files.length,
    changed: files.filter((file) => file.changed).length,
    fixes: files.reduce((total, file) => total + file.fixes.length, 0),
    diagnostics: files.reduce((total, file) => total + file.diagnostics.length, 0),
    failed: files.filter((file) => file.error !== null).length,
  };
  return { schema_version: 1, files, summary };
}
