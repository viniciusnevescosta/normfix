import { decodeUtf8Source } from "./source-text";

export const MAX_FILES = 128;
export const MAX_PATH_BYTES = 240;
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
  | { code: "path_bytes"; count: number }
  | { code: "tar_path" };

export function sourcePathProblem(path: string): SourcePathProblem | null {
  if (
    path.length === 0
    || path.startsWith("/")
    || path.includes("\\")
    || path.includes(":")
    || path.normalize("NFC") !== path
    || path.split("/").some((part) => part === "" || part === "." || part === "..")
    || path.split("/").some((part) => /[. ]$/.test(part) || windowsReservedName(part))
    || [...path].some((character) => /\p{Cc}/u.test(character))
  ) {
    return { code: "portable_path" };
  }
  if (UTF8_ENCODER.encode(path).length > MAX_PATH_BYTES) {
    return { code: "path_bytes", count: MAX_PATH_BYTES };
  }
  if (!portableTarPath(path)) return { code: "tar_path" };

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

function portableTarPath(path: string): boolean {
  if (UTF8_ENCODER.encode(path).length <= 100) return true;
  for (
    let separator = path.lastIndexOf("/");
    separator >= 0;
    separator = path.lastIndexOf("/", separator - 1)
  ) {
    const prefixBytes = UTF8_ENCODER.encode(path.slice(0, separator)).length;
    const nameBytes = UTF8_ENCODER.encode(path.slice(separator + 1)).length;
    if (prefixBytes <= 155 && nameBytes <= 100) return true;
  }
  return false;
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

export class TarArchiveError extends Error {
  readonly code: "path_too_long" | "field_too_long";
  readonly path: string | null;

  constructor(code: "path_too_long" | "field_too_long", path: string | null = null) {
    super(code);
    this.name = "TarArchiveError";
    this.code = code;
    this.path = path;
  }
}

export function buildTar(files: ProjectSourceFile[]): Uint8Array<ArrayBuffer> {
  const chunks: Uint8Array<ArrayBuffer>[] = [];
  for (const file of files) {
    const content = UTF8_ENCODER.encode(file.source);
    const header = new Uint8Array(512);
    const [name, prefix] = splitTarPath(file.path);
    writeTarText(header, 0, 100, name);
    writeTarText(header, 100, 8, "0000644\0");
    writeTarText(header, 108, 8, "0000000\0");
    writeTarText(header, 116, 8, "0000000\0");
    writeTarText(header, 124, 12, `${content.length.toString(8).padStart(11, "0")}\0`);
    writeTarText(header, 136, 12, "00000000000\0");
    header.fill(32, 148, 156);
    header[156] = "0".charCodeAt(0);
    writeTarText(header, 257, 6, "ustar\u0000");
    writeTarText(header, 263, 2, "00");
    writeTarText(header, 265, 32, "normfix");
    writeTarText(header, 297, 32, "normfix");
    writeTarText(header, 345, 155, prefix);
    const checksum = header.reduce((total, byte) => total + byte, 0);
    writeTarText(header, 148, 8, `${checksum.toString(8).padStart(6, "0")}\0 `);
    chunks.push(header, content);
    const padding = (512 - (content.length % 512)) % 512;
    if (padding) chunks.push(new Uint8Array(padding));
  }
  chunks.push(new Uint8Array(1024));
  const size = chunks.reduce((total, chunk) => total + chunk.length, 0);
  const archive = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    archive.set(chunk, offset);
    offset += chunk.length;
  }
  return archive;
}

function splitTarPath(path: string): [string, string] {
  if (UTF8_ENCODER.encode(path).length <= 100) return [path, ""];
  const separators = [...path.matchAll(/\//g)].map((match) => match.index).reverse();
  for (const separator of separators) {
    const prefix = path.slice(0, separator);
    const name = path.slice(separator + 1);
    if (
      UTF8_ENCODER.encode(prefix).length <= 155
      && UTF8_ENCODER.encode(name).length <= 100
    ) {
      return [name, prefix];
    }
  }
  throw new TarArchiveError("path_too_long", path);
}

function writeTarText(
  buffer: Uint8Array<ArrayBuffer>,
  offset: number,
  length: number,
  value: string,
): void {
  const encoded = UTF8_ENCODER.encode(value);
  if (encoded.length > length) throw new TarArchiveError("field_too_long");
  buffer.set(encoded, offset);
}
