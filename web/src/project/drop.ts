// Turning a drop into a list of importable files.
//
// A student drags their project folder, not a hand-picked set of `.c` files.
// That folder contains `.git`, object files, a compiled binary, and a README,
// so the interesting decisions here are what to ignore and how to say what was
// ignored — never silently, and never by refusing the whole drop over one file
// normfix does not format.

import {
  MAX_FILES,
  MAX_FOLDERS,
  MAX_UNSUPPORTED_FILES,
  portablePathProblem,
  sourcePathProblem,
} from "./files";

/** One importable file and the project-relative path it will be stored under. */
export interface DroppedFile {
  path: string;
  file: File;
}

export interface DropSelection {
  files: DroppedFile[];
  /** Explicit directories, including directories with no child entries. */
  folders: string[];
  /**
   * Entries normfix does not format, by path.
   *
   * They are named rather than counted: a project shows what it contains, and
   * a file that vanished on import is one the reader goes looking for.
   */
  unsupported: string[];
  /** Entries rejected for an unsafe path, or beyond the bounded visible list. */
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
function isIgnoredEntry(name: string): boolean {
  return name.startsWith(".");
}

/** Strips the leading slash the entry API puts on every full path. */
function normalizeDropPath(fullPath: string): string {
  return fullPath.replace(/^\/+/, "");
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
 * Takes entry handles while the drop event still owns them.
 *
 * Some browsers expose only `DataTransfer.files`, and compatibility shells can
 * advertise the item API without implementing its non-standard entry method.
 * Both cases deliberately return no entries so the caller can use the plain
 * file fallback instead of losing the drop to a `TypeError`.
 */
export function captureDroppedEntries(items: Iterable<DataTransferItem>): FileSystemEntry[] {
  const entries: FileSystemEntry[] = [];
  for (const item of items) {
    const getEntry = (item as Partial<DataTransferItem>).webkitGetAsEntry;
    if (typeof getEntry !== "function") continue;
    let entry: FileSystemEntry | null = null;
    try {
      entry = getEntry.call(item);
    } catch {
      // Fall back to DataTransfer.files when the compatibility API refuses.
    }
    if (entry !== null) entries.push(entry);
  }
  return entries;
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
  const folders: string[] = [];
  const unsupported: string[] = [];
  let skipped = 0;
  let scanned = 0;

  const walk = async (entry: FileSystemEntryLike, depth: number): Promise<void> => {
    if (scanned >= MAX_ENTRIES_SCANNED || files.length >= MAX_FILES) return;
    scanned += 1;
    if (isIgnoredEntry(entry.name)) return;

    if (entry.isFile) {
      const path = normalizeDropPath(entry.fullPath);
      const problem = sourcePathProblem(path);
      if (problem !== null) {
        if (problem.code === "only_supported" && unsupported.length < MAX_UNSUPPORTED_FILES) {
          unsupported.push(path);
        } else {
          skipped += 1;
        }
        return;
      }
      files.push({ path, file: await readFile(entry as FileEntryLike) });
      return;
    }
    if (!entry.isDirectory) return;
    const folderPath = normalizeDropPath(entry.fullPath);
    if (portablePathProblem(folderPath) !== null || folders.length >= MAX_FOLDERS) {
      skipped += 1;
    } else {
      folders.push(folderPath);
    }
    if (depth >= MAX_DEPTH) return;
    const directory = await readDirectory(
      entry as DirectoryEntryLike,
      MAX_ENTRIES_SCANNED - scanned,
    );
    if (directory.truncated) skipped += 1;
    for (const child of directory.entries) {
      await walk(child, depth + 1);
    }
  };

  for (const entry of entries) {
    if (scanned >= MAX_ENTRIES_SCANNED || files.length >= MAX_FILES) {
      skipped += 1;
      break;
    }
    await walk(entry, 0);
  }
  return { files, folders, unsupported, skipped };
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
async function readDirectory(
  entry: DirectoryEntryLike,
  limit: number,
): Promise<{ entries: FileSystemEntryLike[]; truncated: boolean }> {
  const reader = entry.createReader();
  const all: FileSystemEntryLike[] = [];
  for (;;) {
    const batch = await new Promise<FileSystemEntryLike[]>((resolve, reject) => {
      reader.readEntries(resolve, reject);
    });
    if (batch.length === 0) return { entries: all, truncated: false };
    const available = Math.max(0, limit - all.length);
    all.push(...batch.slice(0, available));
    if (batch.length > available) {
      return { entries: all, truncated: true };
    }
    if (all.length >= limit) {
      const overflow = await new Promise<FileSystemEntryLike[]>((resolve, reject) => {
        reader.readEntries(resolve, reject);
      });
      return { entries: all, truncated: overflow.length > 0 };
    }
  }
}
