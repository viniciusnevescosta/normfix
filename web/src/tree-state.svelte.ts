// What the file panel is currently showing.
//
// The panel is mounted once and reads this. The imperative page keeps its own
// project state and pushes into here after a change, which is the seam that
// lets the two live side by side while the rest of the page moves across: one
// object to update instead of a list to rebuild, and nothing that breaks by
// being forgotten — a stale field shows a stale row, not a panel describing a
// project that no longer exists.
export const identityState = $state({
  email: "",
  stored: false,
  status: "",
  invalid: false,
});

export const diagnosticsState = $state({
  diagnostics: [] as unknown[],
  fixes: [] as unknown[],
  budget: [] as unknown[],
  error: null as string | null,
  stable: true,
});

export const chromeState = $state({
  locale: "en",
  theme: "system",
  stars: null as number | null,
  docsHref: "/docs/",
});

export const codeState = $state({
  formatted: "",
  diff: "",
});

export const headerState = $state({
  path: null as string | null,
  lines: 0,
  bytes: 0,
});

export const confirmState = $state({
  request: null as { text: string } | null,
  accept: null as (() => void) | null,
});

export const statusState = $state({
  runtime: "loading" as "loading" | "ready" | "error",
  runtimeLabel: "",
  offline: "unsupported",
  online: true,
});

export const dragState = $state({
  active: false,
});

export const editorState = $state({
  notice: null as { title: string; detail: string } | null,
});

export const resultState = $state({
  summary: { files: 0, changed: 0, fixes: 0, diagnostics: 0, failed: 0 },
  paths: [] as string[],
  selected: "",
  usable: false,
  applicable: 0,
  diagnosticCount: 0,
  view: "formatted" as "formatted" | "diagnostics" | "diff",
  copyLabel: "",
});

export const treeState = $state({
  files: [] as string[],
  unsupported: new Set<string>(),
  changed: new Set<string>(),
  selected: null as string | null,
});
