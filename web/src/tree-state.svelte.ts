// What the file panel is currently showing.
//
// The panel is mounted once and reads this. The imperative page keeps its own
// project state and pushes into here after a change, which is the seam that
// lets the two live side by side while the rest of the page moves across: one
// object to update instead of a list to rebuild, and nothing that breaks by
// being forgotten — a stale field shows a stale row, not a panel describing a
// project that no longer exists.
export const treeState = $state({
  files: [] as string[],
  unsupported: new Set<string>(),
  changed: new Set<string>(),
  selected: null as string | null,
});
