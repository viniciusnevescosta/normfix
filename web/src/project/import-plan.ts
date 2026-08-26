import { MAX_FILE_BYTES, MAX_UNSUPPORTED_FILES, portablePathKey, sourcePathProblem } from "./files";

export interface ImportableFile {
  readonly size: number;
  arrayBuffer(): Promise<ArrayBuffer>;
}

export interface IncomingFile {
  path: string;
  file: ImportableFile;
}

export type ImportPlanErrorCode = "duplicate" | "conflict" | "file_too_large";

export class ImportPlanError extends Error {
  constructor(
    readonly code: ImportPlanErrorCode,
    readonly path: string,
  ) {
    super(code);
    this.name = "ImportPlanError";
  }
}

export interface ImportPlan {
  candidates: Map<string, readonly [string, ImportableFile]>;
  unsupported: Set<string>;
  ignored: number;
  firstRejected: string | null;
}

/**
 * Plans a whole browser import without changing the open project.
 *
 * Unsupported but portable names can be shown in the tree. Unsafe names never
 * reach DOM state or local storage, and the warning list has a hard ceiling so
 * dropping a build directory cannot create thousands of reactive rows.
 */
export function planImport(
  incoming: readonly IncomingFile[],
  reportedUnsupported: readonly string[],
  loadedPaths: Iterable<string>,
  loadedUnsupported: Iterable<string>,
  alreadySkipped = 0,
): ImportPlan {
  const loadedKeys = new Set([...loadedPaths].map(portablePathKey));
  const unsupported = new Set<string>();
  const unsupportedKeys = new Set<string>();
  for (const path of loadedUnsupported) {
    if (sourcePathProblem(path)?.code !== "only_supported") continue;
    const key = portablePathKey(path);
    if (unsupportedKeys.has(key) || unsupported.size >= MAX_UNSUPPORTED_FILES) continue;
    unsupported.add(path);
    unsupportedKeys.add(key);
  }

  const candidates = new Map<string, readonly [string, ImportableFile]>();
  let ignored = alreadySkipped;
  let firstRejected: string | null = null;

  const rememberUnsupported = (path: string): void => {
    ignored += 1;
    const problem = sourcePathProblem(path);
    if (problem?.code !== "only_supported") {
      firstRejected ??= path;
      return;
    }
    const key = portablePathKey(path);
    if (
      loadedKeys.has(key) ||
      unsupportedKeys.has(key) ||
      unsupported.size >= MAX_UNSUPPORTED_FILES
    ) {
      return;
    }
    unsupported.add(path);
    unsupportedKeys.add(key);
  };

  for (const path of reportedUnsupported) rememberUnsupported(path);

  for (const { path, file } of incoming) {
    const problem = sourcePathProblem(path);
    if (problem !== null) {
      rememberUnsupported(path);
      continue;
    }
    const key = portablePathKey(path);
    if (candidates.has(key)) throw new ImportPlanError("duplicate", path);
    if (loadedKeys.has(key)) throw new ImportPlanError("conflict", path);
    if (file.size > MAX_FILE_BYTES) throw new ImportPlanError("file_too_large", path);
    candidates.set(key, [path, file]);
  }

  return { candidates, unsupported, ignored, firstRejected };
}
