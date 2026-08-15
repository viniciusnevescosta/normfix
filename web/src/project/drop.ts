// Turning a drop into a list of importable files.
//
// A student drags their project folder, not a hand-picked set of `.c` files.
// That folder contains `.git`, object files, a compiled binary, and a README,
// so the interesting decisions here are what to ignore and how to say what was
// ignored — never silently, and never by refusing the whole drop over one file
// normfix does not format.

import { MAX_FILES, sourcePathProblem } from "./files";

/** One importable file and the project-relative path it will be stored under. */
export interface DroppedFile {
  path: string;
  file: File;
}

export interface DropSelection {
  files: DroppedFile[];
  /** How many entries were passed over because normfix does not format them. */
  skipped: number;
}

/**
 * How deep a dropped folder is followed.
 *
 * Well past any 42 project layout, and low enough that a pathological tree
 * cannot keep the page busy indefinitely.
 */
const MAX_DEPTH = 32;

/**
 * How many entries are examined before the walk stops.
 *
 * A dropped `.git` directory alone holds thousands of files. The walk skips it,
 * but the ceiling means no drop can turn into unbounded work regardless of what
 * it contains.
 */
const MAX_ENTRIES_SCANNED = 20_000;

/**
 * Whether an entry is skipped without being looked inside.
 *
 * Dot-entries are the build and tooling residue of a real project — `.git`,
 * `.DS_Store`, `.vscode`. None of it is source a student wants formatted, and
 * `.git` in particular is large enough that walking it would be the slowest
 * part of dropping a project.
 */
export function isIgnoredEntry(name: string): boolean {
  return name.startsWith(".");
}

/** Strips the leading slash the entry API puts on every full path. */
export function normalizeDropPath(fullPath: string): string {
  return fullPath.replace(/^\/+/, "");
}

/** Whether a dropped path is something the playground can format. */
export function isImportablePath(path: string): boolean {
  return sourcePathProblem(path) === null;
}

/** The subset of the entry API this module uses, so the walk can be reasoned about. */
interface FileSystemEntryLike {
  name: string;
  fullPath: string;
  isFile: boolean;
  isDirectory: boolean;
}

interface FileEntryLike extends FileSystemEntryLike {
  file(onSuccess: (file: File) => void, onError: (error: unknown) => void): void;
}

interface DirectoryEntryLike extends FileSystemEntryLike {
  createReader(): {
    readEntries(
      onSuccess: (entries: FileSystemEntryLike[]) => void,
      onError: (error: unknown) => void,
    ): void;
  };
}

/**
 * Reads a drop into importable files.
 *
 * The entries must be taken from the event synchronously — the browser
 * invalidates `DataTransfer.items` as soon as the handler yields — so this
 * takes the already-collected entries rather than the event.
 */
export async function collectDroppedFiles(
  entries: readonly FileSystemEntryLike[],
): Promise<DropSelection> {
  const files: DroppedFile[] = [];
  let skipped = 0;
  let scanned = 0;

  const walk = async (entry: FileSystemEntryLike, depth: number): Promise<void> => {
    if (scanned >= MAX_ENTRIES_SCANNED || files.length >= MAX_FILES) return;
    scanned += 1;
    if (isIgnoredEntry(entry.name)) return;

    if (entry.isFile) {
      const path = normalizeDropPath(entry.fullPath);
      if (!isImportablePath(path)) {
        skipped += 1;
        return;
      }
      files.push({ path, file: await readFile(entry as FileEntryLike) });
      return;
    }
    if (!entry.isDirectory || depth >= MAX_DEPTH) return;
    for (const child of await readDirectory(entry as DirectoryEntryLike)) {
      await walk(child, depth + 1);
    }
  };

  for (const entry of entries) await walk(entry, 0);
  return { files, skipped };
}

function readFile(entry: FileEntryLike): Promise<File> {
  return new Promise((resolve, reject) => {
    entry.file(resolve, reject);
  });
}

/**
 * `readEntries` returns at most one batch per call — typically 100 — and
 * signals the end with an empty result, so a directory has to be drained
 * rather than read once.
 */
async function readDirectory(entry: DirectoryEntryLike): Promise<FileSystemEntryLike[]> {
  const reader = entry.createReader();
  const all: FileSystemEntryLike[] = [];
  for (;;) {
    const batch = await new Promise<FileSystemEntryLike[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    if (batch.length === 0) return all;
    all.push(...batch);
  }
}
