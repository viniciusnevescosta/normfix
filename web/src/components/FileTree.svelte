<script lang="ts">
  // The project panel: folders, files, and everything done to them.
  //
  // The imperative version rebuilt this list by hand and had to remember to do
  // it after every change — a folder opened, a file renamed, a result applied.
  // Forgetting one showed a project that no longer existed. Here the markup is
  // a function of the state, so there is nothing to remember and nothing to
  // forget.
  //
  // The panel owns no project state. It is handed what to draw and hands back
  // what the reader did, which keeps the rules about what a path may be where
  // they are already proven.
  import { SvelteSet } from "svelte/reactivity";

  import { buildTree, type TreeNode } from "../project/tree";

  interface Props {
    files: readonly string[];
    unsupported: ReadonlySet<string>;
    changed: ReadonlySet<string>;
    selected: string | null;
    translate: (key: string) => string;
    kindOf: (path: string) => string;
    onSelect: (path: string) => void;
    onMove: (path: string, isFolder: boolean, folder: string) => void;
    onRename: (path: string, isFolder: boolean) => void;
    onDelete: (path: string, isFolder: boolean) => void;
  }

  const {
    files,
    unsupported,
    changed,
    selected,
    translate,
    kindOf,
    onSelect,
    onMove,
    onRename,
    onDelete,
  }: Props = $props();

  const tree = $derived(buildTree(files));
  let collapsed = $state(new SvelteSet<string>());
  let menu = $state<{ path: string; isFolder: boolean; x: number; y: number } | null>(null);
  let dropInto = $state<string | null>(null);

  /** Flattens the tree to the rows that are actually visible. */
  function rows(nodes: TreeNode[], depth = 0): Array<{ node: TreeNode; depth: number }> {
    const visible: Array<{ node: TreeNode; depth: number }> = [];
    for (const node of nodes) {
      visible.push({ node, depth });
      if (node.kind === "folder" && !collapsed.has(node.path)) {
        visible.push(...rows(node.children, depth + 1));
      }
    }
    return visible;
  }

  function toggle(path: string): void {
    if (collapsed.has(path)) collapsed.delete(path);
    else collapsed.add(path);
  }

  /**
   * Where a drop lands.
   *
   * Dropping on a file means dropping beside it, in the folder that holds it,
   * which is what the pointer looks like it is doing.
   */
  function destination(node: TreeNode): string {
    if (node.kind === "folder") return node.path;
    return node.path.includes("/") ? node.path.slice(0, node.path.lastIndexOf("/")) : "";
  }

  function startDrag(event: DragEvent, node: TreeNode): void {
    event.dataTransfer?.setData("text/normfix-entry", `${node.kind}:${node.path}`);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function drop(event: DragEvent, folder: string): void {
    const payload = event.dataTransfer?.getData("text/normfix-entry");
    dropInto = null;
    if (!payload) return;
    event.preventDefault();
    const separator = payload.indexOf(":");
    onMove(payload.slice(separator + 1), payload.slice(0, separator) === "folder", folder);
  }
</script>

<svelte:window
  onclick={() => (menu = null)}
  onkeydown={(event) => {
    if (event.key === "Escape") menu = null;
  }}
/>

<!-- The panel itself is the project root, so an entry dragged clear of every
     folder comes back out to the top rather than having nowhere to land. -->
<div
  class="flex flex-col overflow-y-auto"
  role="tree"
  tabindex="-1"
  aria-label={translate("loadedFiles")}
  ondragover={(event) => {
    if (event.dataTransfer?.types.includes("text/normfix-entry")) event.preventDefault();
  }}
  ondrop={(event) => drop(event, "")}
>
  {#each rows(tree) as { node, depth } (node.path)}
    {@const isFolder = node.kind === "folder"}
    {@const notFormattable = !isFolder && unsupported.has(node.path)}
    <div
      class="flex items-center gap-2 py-1 pr-2 text-left font-mono text-sm"
      class:bg-surface-selected={node.path === selected}
      class:text-accent={!isFolder && changed.has(node.path)}
      class:opacity-65={notFormattable}
      class:outline-dashed={dropInto === node.path}
      class:cursor-pointer={true}
      style="padding-left: {0.5 + depth * 0.85}rem"
      role="treeitem"
      aria-selected={isFolder ? undefined : node.path === selected}
      aria-level={depth + 1}
      aria-expanded={isFolder ? !collapsed.has(node.path) : undefined}
      tabindex="0"
      draggable="true"
      title={notFormattable ? translate("unsupportedFile") : node.path}
      data-path={node.path}
      data-kind={node.kind}
      ondragstart={(event) => startDrag(event, node)}
      ondragover={(event) => {
        if (!event.dataTransfer?.types.includes("text/normfix-entry")) return;
        event.preventDefault();
        dropInto = isFolder ? node.path : null;
      }}
      ondragleave={() => (dropInto = null)}
      ondrop={(event) => {
        event.stopPropagation();
        drop(event, destination(node));
      }}
      oncontextmenu={(event) => {
        event.preventDefault();
        menu = { path: node.path, isFolder, x: event.clientX, y: event.clientY };
      }}
      onclick={() => (isFolder ? toggle(node.path) : onSelect(node.path))}
      onkeydown={(event) => {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        if (isFolder) toggle(node.path);
        else onSelect(node.path);
      }}
    >
      <!-- The icon is the state: an open folder is open, a closed one closed. -->
      <span aria-hidden="true">
        {isFolder ? (collapsed.has(node.path) ? "\u{1F4C1}" : "\u{1F4C2}") : "•"}
      </span>
      <span class="min-w-0 flex-1 truncate">{node.name}</span>
      {#if !isFolder}
        <span class="text-faint text-xs" class:italic={notFormattable}>
          {notFormattable ? translate("unsupportedKind") : kindOf(node.path)}
        </span>
      {/if}
    </div>
  {/each}
</div>

{#if menu}
  <div
    class="border-border bg-panel-alt fixed z-50 flex min-w-32 flex-col rounded border p-1 shadow-lg"
    role="menu"
    tabindex="-1"
    style="left: {menu.x}px; top: {menu.y}px"
  >
    <button
      class="hover:bg-surface-selected px-2 py-1 text-left text-sm"
      type="button"
      role="menuitem"
      onclick={() => {
        const target = menu;
        menu = null;
        if (target) onRename(target.path, target.isFolder);
      }}
    >
      {translate("renameEntry")}
    </button>
    <button
      class="hover:bg-surface-selected px-2 py-1 text-left text-sm"
      type="button"
      role="menuitem"
      onclick={() => {
        const target = menu;
        menu = null;
        if (target) onDelete(target.path, target.isFolder);
      }}
    >
      {translate("deleteEntry")}
    </button>
  </div>
{/if}
