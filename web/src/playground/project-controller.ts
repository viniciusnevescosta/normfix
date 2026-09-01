import { mount } from "svelte";
import FileTree from "../components/FileTree.svelte";
import type { PlaygroundElements } from "../dom";
import { openDraftRow } from "../project/draft-row";
import { MAX_FOLDERS, portablePathKey, sourcePathProblem } from "../project/files";
import { movedPath, renamedPath, rewritePrefix, wouldContainItself } from "../project/tree";
import {
  confirmState,
  editorState,
  headerState,
  identityState,
  treeState,
} from "../tree-state.svelte";
import type { AppState, Translator } from "./model";
import {
  countFolderEntries,
  editorMeasurements,
  emptyFolderPaths,
  fileKind,
  hasPortablePath,
  normalizeFolderPath,
  normalizeSourcePath,
  validateProjectSources,
} from "./project-model";

export interface ProjectController {
  syncEditor(): void;
  selectFile(path: string, syncCurrent?: boolean): void;
  openDraft(kind: "file" | "folder"): void;
  renderFileList(): void;
  updateEditorMeta(): void;
  validateSources(files: ReadonlyMap<string, string>): void;
  normalizePath(path: string): string;
  removeSelected(): void;
}

export interface ProjectControllerOptions {
  state: AppState;
  elements: PlaygroundElements;
  translate: Translator;
  invalidateResults: () => void;
  refreshRunControl: () => void;
}

export function createProjectController(options: ProjectControllerOptions): ProjectController {
  const { state, elements, translate: t, invalidateResults, refreshRunControl } = options;
  let treeMounted = false;

  function syncEditor(): void {
    if (!state.editor) return;
    const value = state.editor.getValue();
    if (state.selected && state.files.get(state.selected) !== value) {
      state.files.set(state.selected, value);
      state.revision += 1;
      invalidateResults();
    }
  }

  function enableEditor(): void {
    editorState.notice = null;
    refreshRunControl();
  }

  function selectFile(path: string, syncCurrent = true): void {
    if (syncCurrent) syncEditor();
    const source = state.files.get(path);
    if (source === undefined) return;
    enableEditor();
    state.selected = path;
    state.editor?.setFile(path, source);
    headerState.path = path;
    updateEditorMeta();
    renderFileList();
  }

  function openDraft(kind: "file" | "folder"): void {
    if (state.importing) return;
    openDraftRow({
      container: elements.fileList,
      kind,
      label: t(kind === "file" ? "addFile" : "addFolder"),
      create: (path) => (kind === "folder" ? addFolder(path) : addSource(path, "")),
      onClose: () => state.editor?.focus(),
    });
  }

  function applyMoves(moves: Array<readonly [string, string]>): void {
    if (state.importing || moves.length === 0) return;
    syncEditor();
    const proposed = new Map(state.files);
    const proposedUnsupported = new Set(state.unsupported);
    const proposedFolders = new Set(state.folders);
    for (const [from] of moves) {
      proposed.delete(from);
      proposedUnsupported.delete(from);
      proposedFolders.delete(from);
    }
    const occupied = new Set(
      [...proposed.keys(), ...proposedUnsupported, ...proposedFolders].map((path) =>
        portablePathKey(path),
      ),
    );
    for (const [from, to] of moves) {
      const supported = state.files.has(from);
      const folder = state.folders.has(from);
      const problem = sourcePathProblem(to);
      const normalized = folder
        ? normalizeFolderPath(to, t)
        : supported
          ? normalizeSourcePath(to, t)
          : problem?.code === "only_supported"
            ? to
            : (() => {
                if (problem?.code === "path_bytes") {
                  throw new Error(t("pathBytes", { count: problem.count }));
                }
                throw new Error(t(problem === null ? "unsupportedFile" : "portablePath"));
              })();
      const key = portablePathKey(normalized);
      if (occupied.has(key)) throw new Error(t("importConflict", { path: normalized }));
      occupied.add(key);
      if (folder) {
        proposedFolders.add(normalized);
      } else if (supported) {
        const source = state.files.get(from);
        if (source === undefined) throw new Error(t("responsePath", { path: from }));
        proposed.set(normalized, source);
      } else {
        proposedUnsupported.add(normalized);
      }
    }
    if (proposed.size > 0) validateProjectSources(proposed, t);
    const selected = moves.find(([from]) => from === state.selected);
    const shownUnsupported = moves.find(
      ([from]) => from === headerState.path && state.selected === null,
    );
    for (const [from] of moves) {
      if (state.files.has(from)) state.editor?.removeFile(from);
    }
    state.files = proposed;
    state.unsupported = proposedUnsupported;
    state.folders = proposedFolders;
    state.revision += 1;
    invalidateResults();
    const reselect = selected ? normalizeSourcePath(selected[1], t) : state.selected;
    if (shownUnsupported) showUnsupported(shownUnsupported[1]);
    else if (reselect === null) renderFileList();
    else selectFile(reselect, false);
  }

  function moveEntry(path: string, isFolder: boolean, folder: string): void {
    if (isFolder && wouldContainItself(path, folder)) return;
    const paths = [...state.files.keys(), ...state.unsupported, ...state.folders];
    const moves = isFolder
      ? rewritePrefix(paths, path, movedPath(path, folder))
      : [[path, movedPath(path, folder)] as const];
    if (moves.every(([from, to]) => from === to)) return;
    try {
      applyMoves(moves);
    } catch (failure) {
      reportProjectError(failure);
    }
  }

  function renameEntry(path: string, isFolder: boolean, name: string): void {
    const target = renamedPath(path, name);
    if (target === path) return;
    try {
      applyMoves(
        isFolder
          ? rewritePrefix(
              [...state.files.keys(), ...state.unsupported, ...state.folders],
              path,
              target,
            )
          : [[path, target]],
      );
    } catch (failure) {
      reportProjectError(failure);
    }
  }

  function deleteEntry(path: string, isFolder: boolean): void {
    if (state.importing) return;
    syncEditor();
    const under = (loaded: string): boolean =>
      loaded === path || (isFolder && loaded.startsWith(`${path}/`));
    const removed = [...state.files.keys()].filter(under);
    const dropped = [...state.unsupported].filter(under);
    const removedFolders = [...state.folders].filter(under);
    if (removed.length === 0 && dropped.length === 0 && removedFolders.length === 0) return;
    const proposed = new Map(state.files);
    for (const loaded of removed) {
      proposed.delete(loaded);
      state.editor?.removeFile(loaded);
    }
    state.files = proposed;
    for (const loaded of dropped) state.unsupported.delete(loaded);
    for (const folder of removedFolders) state.folders.delete(folder);
    state.revision += 1;
    invalidateResults();
    const showingRemoved =
      (state.selected !== null && removed.includes(state.selected)) ||
      (headerState.path !== null && dropped.includes(headerState.path));
    if (showingRemoved) {
      const next = [...state.files.keys()].sort()[0];
      if (next === undefined) showEmptyProject();
      else selectFile(next, false);
    } else if (state.files.size === 0) {
      showEmptyProject();
    } else {
      renderFileList();
    }
  }

  function showEmptyProject(): void {
    state.selected = null;
    headerState.path = null;
    editorState.notice = { title: t("noFilesTitle"), detail: t("emptyProjectHint") };
    refreshRunControl();
    renderFileList();
  }

  function reportProjectError(failure: unknown): void {
    identityState.status = failure instanceof Error ? failure.message : String(failure);
    identityState.invalid = false;
  }

  function renderFileList(): void {
    treeState.files = [...state.files.keys(), ...state.unsupported];
    treeState.folders = new Set(state.folders);
    treeState.unsupported = new Set(state.unsupported);
    treeState.changed = new Set(
      [...state.results.entries()].filter(([, result]) => result.changed).map(([path]) => path),
    );
    treeState.selected = state.selected;
    renderEmptyFolderNotice();
    if (treeMounted) return;
    treeMounted = true;
    mount(FileTree, {
      target: elements.fileList,
      props: {
        get files() {
          return treeState.files;
        },
        get folders() {
          return treeState.folders;
        },
        get unsupported() {
          return treeState.unsupported;
        },
        get changed() {
          return treeState.changed;
        },
        get selected() {
          return treeState.selected;
        },
        translate: (key: string) => t(key as Parameters<Translator>[0]),
        kindOf: fileKind,
        onSelect: (path: string) => {
          if (state.unsupported.has(path)) showUnsupported(path);
          else selectFile(path);
        },
        onMove: moveEntry,
        onRename: startRename,
        onDelete: confirmDelete,
      },
    });
  }

  function renderEmptyFolderNotice(): void {
    const empty = emptyFolderPaths(state.folders, state.files.keys(), state.unsupported);
    elements.emptyFolderNotice.hidden = empty.length === 0;
    elements.emptyFolderNotice.textContent =
      empty.length === 0
        ? ""
        : t(empty.length === 1 ? "emptyFolderWarningOne" : "emptyFolderWarningOther", {
            count: empty.length,
            paths: empty.slice(0, 3).join(", "),
          });
  }

  function showUnsupported(path: string): void {
    syncEditor();
    state.selected = null;
    headerState.path = path;
    editorState.notice = { title: t("unsupportedFile"), detail: t("supportedKinds") };
    refreshRunControl();
    renderFileList();
  }

  function confirmDelete(path: string, isFolder: boolean): void {
    const count = isFolder ? countFolderEntries(state.files.keys(), state.unsupported, path) : 1;
    confirmState.request = {
      text: isFolder
        ? t("deleteFolderText", { path, count: String(count) })
        : t("deleteFileText", { path }),
    };
    confirmState.accept = () => deleteEntry(path, isFolder);
  }

  function updateEditorMeta(): void {
    const source = state.editor?.getValue() ?? state.files.get(state.selected ?? "") ?? "";
    const measurements = editorMeasurements(source);
    headerState.lines = measurements.lines;
    headerState.bytes = measurements.bytes;
  }

  function validateSources(files: ReadonlyMap<string, string>): void {
    validateProjectSources(files, t);
  }

  function normalizePath(path: string): string {
    return normalizeSourcePath(path, t);
  }

  function addSource(path: string, source = ""): void {
    if (state.importing) throw new Error(t("waitForImport"));
    syncEditor();
    const normalized = normalizeSourcePath(path, t);
    if (
      hasPortablePath([...state.files.keys(), ...state.unsupported, ...state.folders], normalized)
    ) {
      throw new Error(t("importConflict", { path: normalized }));
    }
    const proposed = new Map(state.files);
    proposed.set(normalized, source);
    validateProjectSources(proposed, t);
    state.files = proposed;
    state.revision += 1;
    invalidateResults();
    selectFile(normalized, false);
  }

  function addFolder(path: string): void {
    if (state.importing) throw new Error(t("waitForImport"));
    if (state.folders.size >= MAX_FOLDERS) {
      throw new Error(t("maxFolders", { count: MAX_FOLDERS }));
    }
    syncEditor();
    const normalized = normalizeFolderPath(path, t);
    if (
      hasPortablePath([...state.files.keys(), ...state.unsupported, ...state.folders], normalized)
    ) {
      throw new Error(t("importConflict", { path: normalized }));
    }
    state.folders = new Set(state.folders).add(normalized);
    state.revision += 1;
    invalidateResults();
    renderFileList();
  }

  function removeSelected(): void {
    if (state.importing || !state.selected) return;
    confirmDelete(state.selected, false);
  }

  function startRename(path: string, isFolder: boolean): void {
    const entry = elements.fileList.querySelector<HTMLElement>(`[data-path="${CSS.escape(path)}"]`);
    const label = entry?.querySelector<HTMLElement>("[data-entry-name]");
    if (!label) return;
    const current = label.textContent ?? "";
    const input = document.createElement("input");
    input.className = "file-name";
    input.value = current;
    input.setAttribute("aria-label", t("renameEntry"));
    label.replaceWith(input);
    input.focus();
    input.select();

    let settled = false;
    const restore = (): void => {
      if (settled) return;
      settled = true;
      renderFileList();
    };
    input.addEventListener("keydown", (event) => {
      if (event.key === "Escape") restore();
      if (event.key !== "Enter") return;
      event.preventDefault();
      const typed = input.value.trim();
      settled = true;
      if (typed.length === 0 || typed === current) {
        renderFileList();
        return;
      }
      renameEntry(path, isFolder, typed);
      renderFileList();
    });
    input.addEventListener("blur", restore);
  }

  return {
    syncEditor,
    selectFile,
    openDraft,
    renderFileList,
    updateEditorMeta,
    validateSources,
    normalizePath,
    removeSelected,
  };
}
