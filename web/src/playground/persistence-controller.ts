import type { PlaygroundElements } from "../dom";
import { deserializeProject, isSameProject, serializeProject } from "../project/persistence";
import { editorState, headerState } from "../tree-state.svelte";
import type { AppState, Translator } from "./model";
import { PROJECT_STORAGE_KEY, SAMPLE } from "./model";

export interface PersistenceController {
  attach(): void;
  scheduleSave(): void;
  restoreProject(): void;
}

export interface PersistenceControllerOptions {
  state: AppState;
  elements: PlaygroundElements;
  translate: Translator;
  refreshRunControl: () => void;
  invalidateResults: () => void;
  selectFile: (path: string, syncCurrent?: boolean) => void;
}

export function createPersistenceController(
  options: PersistenceControllerOptions,
): PersistenceController {
  const {
    state,
    elements,
    translate: t,
    refreshRunControl,
    invalidateResults,
    selectFile,
  } = options;
  let saveTimer: number | null = null;
  let attached = false;

  function saveProject(): void {
    if (saveTimer !== null) {
      window.clearTimeout(saveTimer);
      saveTimer = null;
    }
    const payload = serializeProject({
      files: Object.fromEntries(state.files),
      folders: [...state.folders],
      selected: state.selected,
      unsupported: [...state.unsupported],
      savedAt: Date.now(),
    });
    try {
      if (payload === null) localStorage.removeItem(PROJECT_STORAGE_KEY);
      else localStorage.setItem(PROJECT_STORAGE_KEY, payload);
    } catch {
      // The project is still open; storage is only a recovery aid.
    }
  }

  function scheduleSave(): void {
    if (saveTimer !== null) window.clearTimeout(saveTimer);
    saveTimer = window.setTimeout(saveProject, 600);
  }

  function clearScheduledSave(): void {
    if (saveTimer === null) return;
    window.clearTimeout(saveTimer);
    saveTimer = null;
  }

  function readStoredProject(): string | null {
    try {
      return localStorage.getItem(PROJECT_STORAGE_KEY);
    } catch {
      return null;
    }
  }

  function restoreProject(): void {
    const stored = deserializeProject(readStoredProject());
    if (!stored || isSameProject(stored, state.files, state.folders, state.unsupported)) return;
    state.files = new Map(Object.entries(stored.files));
    state.folders = new Set(stored.folders);
    state.unsupported = new Set(stored.unsupported);
    state.revision += 1;
    const selected =
      stored.selected !== null && state.files.has(stored.selected)
        ? stored.selected
        : [...state.files.keys()].sort()[0];
    if (selected === undefined) {
      state.selected = null;
      headerState.path = null;
      editorState.notice = { title: t("noFilesTitle"), detail: t("emptyProjectHint") };
      refreshRunControl();
    } else {
      state.selected = selected;
      headerState.path = selected;
    }
    elements.restoreNotice.hidden = false;
  }

  function discardStoredProject(): void {
    state.files = new Map([["main.c", SAMPLE]]);
    state.folders = new Set();
    state.unsupported = new Set();
    state.revision += 1;
    invalidateResults();
    clearScheduledSave();
    try {
      localStorage.removeItem(PROJECT_STORAGE_KEY);
    } catch {
      // Nothing stored is the requested state either way.
    }
    selectFile("main.c", false);
    elements.restoreNotice.hidden = true;
  }

  function attach(): void {
    if (attached) return;
    attached = true;
    elements.discardRestore.addEventListener("click", discardStoredProject);
    addEventListener("pagehide", saveProject);
    addEventListener("visibilitychange", () => {
      if (document.visibilityState === "hidden") saveProject();
    });
  }

  return { attach, scheduleSave, restoreProject };
}
