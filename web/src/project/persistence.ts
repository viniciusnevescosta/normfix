// Keeping a project across a reload, without keeping it a secret.
//
// Closing a tab by accident should not cost an afternoon, so the project is
// written to this browser as it changes. But this page is built for 42 campus
// machines, which are shared: code left behind is code the next person at that
// keyboard opens. So restoring is never silent — the page says the work came
// back and offers to drop it — and nothing is ever written anywhere but here.

import {
  MAX_FILE_BYTES,
  MAX_FILES,
  MAX_FOLDERS,
  MAX_PROJECT_BYTES,
  MAX_UNSUPPORTED_FILES,
  portablePathKey,
  portablePathProblem,
  sourcePathProblem,
} from "./files";

/** What a stored project holds. */
export interface StoredProject {
  /** Path to source, exactly as the project held them. */
  files: Record<string, string>;
  /** Explicit directories, including empty ones. */
  folders: string[];
  /** Which file was open. */
  selected: string | null;
  /** Paths the project showed but could not format. */
  unsupported: string[];
  /** When it was written, so a restore can say how old it is. */
  savedAt: number;
}

/**
 * How much stored work is worth keeping.
 *
 * Browser storage is a few megabytes shared with everything else this origin
 * keeps, and a project over this is one the reader imported rather than typed
 * — recoverable from where it came from, unlike work done here.
 */
export const MAX_STORED_BYTES = 2 * 1024 * 1024;

const ENCODER = new TextEncoder();

/**
 * Turns a project into what gets stored, or `null` when it is not worth it.
 *
 * An empty project stores nothing: restoring nothing over nothing only means
 * telling the reader their work came back when it did not.
 */
export function serializeProject(project: StoredProject): string | null {
  if (
    Object.keys(project.files).length === 0 &&
    project.unsupported.length === 0 &&
    project.folders.length === 0
  ) {
    return null;
  }
  const payload = JSON.stringify(project);
  return ENCODER.encode(payload).length > MAX_STORED_BYTES ? null : payload;
}

/**
 * Reads back what was stored, or `null` when there is nothing usable.
 *
 * Anything unreadable is treated as absent rather than repaired: a project
 * half-recovered from damaged storage is worse than one the reader knows they
 * have to open again.
 */
export function deserializeProject(payload: string | null): StoredProject | null {
  if (!payload) return null;
  if (ENCODER.encode(payload).length > MAX_STORED_BYTES) return null;
  let parsed: unknown;
  try {
    parsed = JSON.parse(payload);
  } catch {
    return null;
  }
  if (typeof parsed !== "object" || parsed === null) return null;
  const record = parsed as Partial<StoredProject>;
  const files = record.files;
  if (typeof files !== "object" || files === null) return null;
  if (Array.isArray(files)) return null;
  const entries = Object.entries(files);
  if (entries.length > MAX_FILES) return null;
  const portable = new Set<string>();
  let projectBytes = 0;
  for (const [path, source] of entries) {
    if (typeof source !== "string" || sourcePathProblem(path) !== null) return null;
    const key = portablePathKey(path);
    if (portable.has(key)) return null;
    portable.add(key);
    const bytes = ENCODER.encode(source).length;
    if (bytes > MAX_FILE_BYTES) return null;
    projectBytes += bytes;
    if (projectBytes > MAX_PROJECT_BYTES) return null;
  }
  if (!Array.isArray(record.unsupported)) return null;
  const unsupported: string[] = [];
  for (const path of record.unsupported) {
    if (typeof path !== "string" || sourcePathProblem(path)?.code !== "only_supported") {
      return null;
    }
    const key = portablePathKey(path);
    if (portable.has(key)) return null;
    if (unsupported.length >= MAX_UNSUPPORTED_FILES) return null;
    portable.add(key);
    unsupported.push(path);
  }
  if (record.folders !== undefined && !Array.isArray(record.folders)) return null;
  const folders: string[] = [];
  for (const path of record.folders ?? []) {
    if (typeof path !== "string" || portablePathProblem(path) !== null) return null;
    const key = portablePathKey(path);
    if (portable.has(key) || folders.length >= MAX_FOLDERS) return null;
    portable.add(key);
    folders.push(path);
  }
  if (entries.length === 0 && unsupported.length === 0 && folders.length === 0) return null;
  const selected =
    typeof record.selected === "string" && entries.some(([path]) => path === record.selected)
      ? record.selected
      : null;
  return {
    files: Object.fromEntries(entries as [string, string][]),
    folders,
    selected,
    unsupported,
    savedAt:
      typeof record.savedAt === "number" && Number.isFinite(record.savedAt) && record.savedAt >= 0
        ? record.savedAt
        : 0,
  };
}

/**
 * Whether a restored project is the one already on screen.
 *
 * The page starts with one sample file. Announcing a restore that produced
 * exactly that would be announcing nothing.
 */
export function isSameProject(
  stored: StoredProject,
  files: ReadonlyMap<string, string>,
  folders: ReadonlySet<string> = new Set(),
  unsupported: ReadonlySet<string> = new Set(),
): boolean {
  const entries = Object.entries(stored.files);
  if (entries.length !== files.size) return false;
  return (
    entries.every(([path, source]) => files.get(path) === source) &&
    stored.folders.length === folders.size &&
    stored.folders.every((path) => folders.has(path)) &&
    stored.unsupported.length === unsupported.size &&
    stored.unsupported.every((path) => unsupported.has(path))
  );
}
