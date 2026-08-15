// The shape a project has on disk, recovered from the paths it stores.
//
// This project stores flat paths, because that is what a formatter is given and
// what an archive writes back. A reader does not think in flat paths, though:
// they dropped a folder in, and they expect to see the folder. These functions
// derive the one from the other, and say what a move or a rename does to a path
// — the parts worth testing without a page around them.

/** One entry in the derived tree. */
export type TreeNode =
  | { kind: "file"; name: string; path: string }
  | { kind: "folder"; name: string; path: string; children: TreeNode[] };

/**
 * Builds the folder tree the given paths imply.
 *
 * Folders are sorted before files and each group by name, so a project reads
 * the way a file browser shows it rather than the way a map happened to store
 * it.
 */
export function buildTree(paths: Iterable<string>): TreeNode[] {
  const root: TreeNode[] = [];
  for (const path of [...paths].sort()) {
    const segments = path.split("/");
    let level = root;
    let prefix = "";
    for (const [index, segment] of segments.entries()) {
      prefix = prefix === "" ? segment : `${prefix}/${segment}`;
      if (index === segments.length - 1) {
        level.push({ kind: "file", name: segment, path });
        continue;
      }
      const existing = level.find(
        (node): node is Extract<TreeNode, { kind: "folder" }> =>
          node.kind === "folder" && node.name === segment,
      );
      if (existing) {
        level = existing.children;
        continue;
      }
      const folder = { kind: "folder" as const, name: segment, path: prefix, children: [] };
      level.push(folder);
      level = folder.children;
    }
  }
  return sortLevel(root);
}

function sortLevel(level: TreeNode[]): TreeNode[] {
  level.sort((left, right) => {
    if (left.kind !== right.kind) return left.kind === "folder" ? -1 : 1;
    return left.name.localeCompare(right.name, "en");
  });
  for (const node of level) {
    if (node.kind === "folder") sortLevel(node.children);
  }
  return level;
}

/**
 * Where `path` ends up when dropped on `folder`.
 *
 * An empty folder means the project root. Dropping something where it already
 * is returns the same path, so a caller can tell a real move from a nudge.
 */
export function movedPath(path: string, folder: string): string {
  const name = path.split("/").at(-1) ?? path;
  return folder === "" ? name : `${folder}/${name}`;
}

/**
 * Whether `folder` is inside `path`, which is the move that cannot be made.
 *
 * Dragging a folder into its own child would put a folder inside itself: the
 * paths under it would have to contain their own prefix forever.
 */
export function wouldContainItself(path: string, folder: string): boolean {
  return folder === path || folder.startsWith(`${path}/`);
}

/** The path `path` takes when its last segment is renamed to `name`. */
export function renamedPath(path: string, name: string): string {
  const segments = path.split("/");
  segments[segments.length - 1] = name;
  return segments.join("/");
}

/**
 * Every path that moves when the folder `from` is renamed or moved to `to`.
 *
 * A folder is only a prefix here, so renaming one is rewriting the prefix of
 * everything beneath it — returned as pairs rather than applied, because the
 * caller owns the project and has to check the result before taking it.
 */
export function rewritePrefix(
  paths: Iterable<string>,
  from: string,
  to: string,
): Array<readonly [string, string]> {
  const moves: Array<readonly [string, string]> = [];
  for (const path of paths) {
    if (path === from) {
      moves.push([path, to]);
    } else if (path.startsWith(`${from}/`)) {
      moves.push([path, `${to}${path.slice(from.length)}`]);
    }
  }
  return moves;
}
