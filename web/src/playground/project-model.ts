import {
  MAX_FILE_BYTES,
  MAX_FILES,
  MAX_PROJECT_BYTES,
  portablePathKey,
  sourcePathProblem,
} from "../project/files";
import type { Translator } from "./model";

const UTF8_ENCODER = new TextEncoder();

export function fileKind(path: string): string {
  const filename = path.split("/").at(-1)?.toLowerCase() ?? "";
  if (filename === "makefile") return "MK";
  if (filename.endsWith(".md")) return "MD";
  if (filename.endsWith(".h")) return "H";
  return "C";
}

export function normalizeSourcePath(path: string, t: Translator): string {
  const problem = sourcePathProblem(path);
  if (!problem) return path;
  if (problem.code === "only_supported") throw new Error(t("onlySupported"));
  if (problem.code === "path_bytes") {
    throw new Error(t("pathBytes", { count: problem.count }));
  }
  throw new Error(t("portablePath"));
}

export function hasPortablePath(paths: Iterable<string>, candidate: string): boolean {
  const key = portablePathKey(candidate);
  for (const path of paths) {
    if (portablePathKey(path) === key) return true;
  }
  return false;
}

export function validateProjectSources(files: ReadonlyMap<string, string>, t: Translator): void {
  if (files.size === 0) throw new Error(t("emptyProject"));
  if (files.size > MAX_FILES) {
    throw new Error(t("maxFiles", { count: MAX_FILES }));
  }
  let projectBytes = 0;
  const portablePaths = new Set<string>();
  for (const [path, source] of files) {
    const normalized = normalizeSourcePath(path, t);
    const key = portablePathKey(normalized);
    if (portablePaths.has(key)) {
      throw new Error(t("pathCollision", { path }));
    }
    portablePaths.add(key);
    const fileBytes = UTF8_ENCODER.encode(source).length;
    if (fileBytes > MAX_FILE_BYTES) {
      throw new Error(t("fileTooLarge", { path, count: MAX_FILE_BYTES }));
    }
    projectBytes += fileBytes;
    if (projectBytes > MAX_PROJECT_BYTES) {
      throw new Error(t("projectTooLarge", { count: MAX_PROJECT_BYTES }));
    }
  }
}

export function editorMeasurements(source: string): { lines: number; bytes: number } {
  return {
    lines: source.length === 0 ? 0 : source.split("\n").length,
    bytes: UTF8_ENCODER.encode(source).length,
  };
}

export function countFolderEntries(
  supported: Iterable<string>,
  unsupported: Iterable<string>,
  folder: string,
): number {
  const prefix = `${folder}/`;
  let count = 0;
  for (const path of supported) {
    if (path.startsWith(prefix)) count += 1;
  }
  for (const path of unsupported) {
    if (path.startsWith(prefix)) count += 1;
  }
  return count;
}
