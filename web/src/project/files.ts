import { decodeUtf8Source } from "./source-text";

export const MAX_FILES = 128;
const MAX_PATH_BYTES = 240;
export const MAX_FILE_BYTES = 1024 * 1024;
export const MAX_PROJECT_BYTES = 4 * 1024 * 1024;

const UTF8_ENCODER = new TextEncoder();

export interface ProjectSourceFile {
  path: string;
  source: string;
}

export type SourcePathProblem =
  | { code: "only_supported" }
  | { code: "portable_path" }
  | { code: "path_bytes"; count: number };

export function sourcePathProblem(path: string): SourcePathProblem | null {
  if (
    path.length === 0 ||
    path.startsWith("/") ||
    path.includes("\\") ||
    path.includes(":") ||
    path.normalize("NFC") !== path ||
    path.split("/").some((part) => part === "" || part === "." || part === "..") ||
    path.split("/").some((part) => /[. ]$/.test(part) || windowsReservedName(part)) ||
    [...path].some((character) => /\p{Cc}/u.test(character))
  ) {
    return { code: "portable_path" };
  }
  if (UTF8_ENCODER.encode(path).length > MAX_PATH_BYTES) {
    return { code: "path_bytes", count: MAX_PATH_BYTES };
  }
  const filename = path.split("/").at(-1)?.toLowerCase() ?? "";
  if (filename !== "makefile" && !/\.(c|h|md)$/.test(filename)) {
    return { code: "only_supported" };
  }
  return null;
}

export function portablePathKey(path: string): string {
  return path.normalize("NFC").toLocaleLowerCase("en-US");
}

export function canonicalIdentityEmail(value: string): string | null {
  const email = value.trim().toLowerCase();
  return /^([a-z0-9][a-z0-9._-]*)@(42\.fr|student\.42[a-z0-9-]*(?:\.[a-z0-9-]+)+)$/.test(email)
    ? email
    : null;
}

function windowsReservedName(segment: string): boolean {
  const stem = segment.split(".")[0]?.toUpperCase() ?? "";
  return /^(?:CON|PRN|AUX|NUL|COM[1-9]|LPT[1-9])$/.test(stem);
}

interface ReadableImportFile {
  arrayBuffer(): Promise<ArrayBuffer>;
}

export type ImportCandidate = readonly [string, ReadableImportFile];

export class ImportBatchError extends Error {
  readonly code: "invalid_utf8" | "project_changed";
  readonly path: string | null;

  constructor(code: "invalid_utf8" | "project_changed", path: string | null = null) {
    super(code);
    this.name = "ImportBatchError";
    this.code = code;
    this.path = path;
  }
}

export async function readImportBatch(
  candidates: Iterable<ImportCandidate>,
  expectedRevision: number,
  currentRevision: () => number,
): Promise<{ sources: Map<string, string>; selectedPath: string | null }> {
  assertRevision(expectedRevision, currentRevision);
  const sources = new Map<string, string>();
  let selectedPath: string | null = null;
  for (const [path, file] of candidates) {
    let source: string;
    try {
      source = decodeUtf8Source(await file.arrayBuffer());
    } catch {
      throw new ImportBatchError("invalid_utf8", path);
    }
    assertRevision(expectedRevision, currentRevision);
    sources.set(path, source);
    selectedPath = path;
  }
  assertRevision(expectedRevision, currentRevision);
  return { sources, selectedPath };
}

function assertRevision(expectedRevision: number, currentRevision: () => number): void {
  if (currentRevision() !== expectedRevision) {
    throw new ImportBatchError("project_changed");
  }
}
