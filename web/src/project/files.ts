import { decodeUtf8Source } from "./source-text";

export const MAX_FILES = 128;
const MAX_PATH_BYTES = 240;
export const MAX_FILE_BYTES = 1024 * 1024;
export const MAX_PROJECT_BYTES = 4 * 1024 * 1024;
/** Explicit directories kept by the browser project, including empty ones. */
export const MAX_FOLDERS = 256;
/** Non-source paths shown as warnings, bounded so a build tree cannot freeze the UI. */
export const MAX_UNSUPPORTED_FILES = 384;

const UTF8_ENCODER = new TextEncoder();
const MAX_CONCURRENT_READS = 4;

export interface ProjectSourceFile {
  path: string;
  source: string;
}

export type SourcePathProblem =
  | { code: "only_supported" }
  | { code: "portable_path" }
  | { code: "path_bytes"; count: number };

export type PortablePathProblem = Exclude<SourcePathProblem, { code: "only_supported" }>;

/** Validates the filesystem-safe part shared by file and directory paths. */
export function portablePathProblem(path: string): PortablePathProblem | null {
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
  return null;
}

export function sourcePathProblem(path: string): SourcePathProblem | null {
  const portableProblem = portablePathProblem(path);
  if (portableProblem) return portableProblem;
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
  readonly code: "invalid_utf8" | "file_too_large" | "project_too_large" | "project_changed";
  readonly path: string | null;

  constructor(
    code: "invalid_utf8" | "file_too_large" | "project_too_large" | "project_changed",
    path: string | null = null,
  ) {
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
  const batch = [...candidates];
  const decoded = new Array<string>(batch.length);
  let next = 0;
  let decodedBytes = 0;
  const readNext = async (): Promise<void> => {
    for (;;) {
      const index = next;
      next += 1;
      const candidate = batch[index];
      if (!candidate) return;
      const [path, file] = candidate;
      try {
        const bytes = await file.arrayBuffer();
        if (bytes.byteLength > MAX_FILE_BYTES) {
          throw new ImportBatchError("file_too_large", path);
        }
        decodedBytes += bytes.byteLength;
        if (decodedBytes > MAX_PROJECT_BYTES) {
          throw new ImportBatchError("project_too_large", path);
        }
        decoded[index] = decodeUtf8Source(bytes);
      } catch (error) {
        if (error instanceof ImportBatchError) throw error;
        throw new ImportBatchError("invalid_utf8", path);
      }
      assertRevision(expectedRevision, currentRevision);
    }
  };
  await Promise.all(
    Array.from({ length: Math.min(MAX_CONCURRENT_READS, batch.length) }, () => readNext()),
  );
  assertRevision(expectedRevision, currentRevision);
  const sources = new Map(
    batch.map(([path], index) => {
      const source = decoded[index];
      if (source === undefined) throw new ImportBatchError("invalid_utf8", path);
      return [path, source] as const;
    }),
  );
  const selectedPath = batch.at(-1)?.[0] ?? null;
  return { sources, selectedPath };
}

function assertRevision(expectedRevision: number, currentRevision: () => number): void {
  if (currentRevision() !== expectedRevision) {
    throw new ImportBatchError("project_changed");
  }
}
