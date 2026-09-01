import type { PlaygroundElements } from "../dom";
import { captureDroppedEntries, collectDroppedFiles, type DroppedFile } from "../project/drop";
import {
  ImportBatchError,
  MAX_FILE_BYTES,
  MAX_FILES,
  MAX_PROJECT_BYTES,
  readImportBatch,
} from "../project/files";
import { ImportPlanError, planImport } from "../project/import-plan";
import { dragState } from "../tree-state.svelte";
import type { AppState, PluralTranslator, Translator } from "./model";

const UTF8_ENCODER = new TextEncoder();

export interface ImportController {
  attach(): void;
  setControls(disabled: boolean): void;
}

export interface ImportControllerOptions {
  state: AppState;
  elements: PlaygroundElements;
  translate: Translator;
  translatePlural: PluralTranslator;
  setRuntime: (state: "loading" | "ready" | "error", label: string) => void;
  refreshRunControl: () => void;
  syncEditor: () => void;
  normalizePath: (path: string) => string;
  validateSources: (files: ReadonlyMap<string, string>) => void;
  invalidateResults: () => void;
  selectFile: (path: string, syncCurrent?: boolean) => void;
  renderFileList: () => void;
}

export function createImportController(options: ImportControllerOptions): ImportController {
  const {
    state,
    elements,
    translate: t,
    translatePlural: tPlural,
    setRuntime,
    refreshRunControl,
    syncEditor,
    normalizePath,
    validateSources,
    invalidateResults,
    selectFile,
    renderFileList,
  } = options;
  let dragDepth = 0;
  let attached = false;

  function setControls(disabled: boolean): void {
    elements.filePicker.disabled = disabled;
    elements.addFile.disabled = disabled;
    elements.addFolder.disabled = disabled;
    elements.removeFile.disabled = disabled;
    refreshRunControl();
  }

  async function loadFiles(
    incoming: readonly DroppedFile[],
    unsupported: readonly string[] = [],
    folders: readonly string[] = [],
    alreadySkipped = 0,
  ): Promise<void> {
    if (
      incoming.length === 0 &&
      unsupported.length === 0 &&
      folders.length === 0 &&
      alreadySkipped === 0
    ) {
      return;
    }
    if (state.importing) throw new Error(t("importRunning"));
    syncEditor();
    const startingRevision = state.revision;
    state.importing = true;
    setControls(true);
    try {
      let plan: ReturnType<typeof planImport>;
      try {
        plan = planImport(
          incoming,
          unsupported,
          folders,
          state.files.keys(),
          state.unsupported,
          state.folders,
          alreadySkipped,
        );
      } catch (error) {
        if (!(error instanceof ImportPlanError)) throw error;
        if (error.code === "duplicate") throw new Error(t("importDuplicate", { path: error.path }));
        if (error.code === "conflict") throw new Error(t("importConflict", { path: error.path }));
        throw new Error(t("fileTooLarge", { path: error.path, count: MAX_FILE_BYTES }));
      }
      const { candidates } = plan;
      const unsupportedChanged =
        plan.unsupported.size !== state.unsupported.size ||
        [...plan.unsupported].some((path) => !state.unsupported.has(path));
      const newFolderCount = [...plan.folders].filter((path) => !state.folders.has(path)).length;
      if (candidates.size === 0 && !unsupportedChanged && newFolderCount === 0) {
        if (plan.firstRejected !== null) normalizePath(plan.firstRejected);
        throw new Error(t("onlySupported"));
      }
      if (state.files.size + candidates.size > MAX_FILES) {
        throw new Error(t("maxFiles", { count: MAX_FILES }));
      }
      const projectedBytes = [...state.files.values()].reduce(
        (total, source) => total + UTF8_ENCODER.encode(source).length,
        [...candidates.values()].reduce((total, [, file]) => total + file.size, 0),
      );
      if (projectedBytes > MAX_PROJECT_BYTES) {
        throw new Error(t("projectTooLarge", { count: MAX_PROJECT_BYTES }));
      }

      let imported: Awaited<ReturnType<typeof readImportBatch>>;
      try {
        imported = await readImportBatch(
          candidates.values(),
          startingRevision,
          () => state.revision,
        );
      } catch (error) {
        if (error instanceof ImportBatchError && error.code === "project_changed") {
          throw new Error(t("importChanged"));
        }
        if (error instanceof ImportBatchError && error.code === "file_too_large" && error.path) {
          throw new Error(t("fileTooLarge", { path: error.path, count: MAX_FILE_BYTES }));
        }
        if (error instanceof ImportBatchError && error.code === "project_too_large") {
          throw new Error(t("projectTooLarge", { count: MAX_PROJECT_BYTES }));
        }
        if (error instanceof ImportBatchError && error.path) {
          throw new Error(t("invalidUtf8", { path: error.path }));
        }
        throw error;
      }
      const proposed = new Map(state.files);
      for (const [path, source] of imported.sources) proposed.set(path, source);
      validateSources(proposed);
      if (state.revision !== startingRevision) throw new Error(t("importChanged"));
      state.files = proposed;
      state.unsupported = plan.unsupported;
      state.folders = plan.folders;
      state.revision += 1;
      invalidateResults();
      if (imported.selectedPath) selectFile(imported.selectedPath, false);
      renderFileList();
      const messages: string[] = [];
      if (candidates.size > 0) messages.push(tPlural("imported", candidates.size));
      if (newFolderCount > 0) messages.push(tPlural("foldersImported", newFolderCount));
      if (plan.ignored > 0) messages.push(tPlural("skipped", plan.ignored));
      setRuntime("ready", messages.join(" "));
    } finally {
      state.importing = false;
      setControls(false);
      elements.filePicker.value = "";
    }
  }

  function dragCarriesFiles(event: DragEvent): boolean {
    return [...(event.dataTransfer?.types ?? [])].includes("Files");
  }

  function setDragging(active: boolean): void {
    if (!active) dragDepth = 0;
    dragState.active = active;
  }

  async function importDrop(transfer: DataTransfer | null): Promise<void> {
    if (!transfer) return;
    const entries = captureDroppedEntries(transfer.items);
    if (entries.length === 0) {
      await loadFiles([...transfer.files].map((file) => ({ path: file.name, file })));
      return;
    }
    const selection = await collectDroppedFiles(entries);
    await loadFiles(selection.files, selection.unsupported, selection.folders, selection.skipped);
  }

  function reportFailure(error: unknown): void {
    setRuntime("error", error instanceof Error ? error.message : String(error));
  }

  function attach(): void {
    if (attached) return;
    attached = true;
    window.addEventListener("dragenter", (event) => {
      if (!dragCarriesFiles(event)) return;
      event.preventDefault();
      dragDepth += 1;
      dragState.active = true;
    });
    window.addEventListener("dragover", (event) => {
      if (!dragCarriesFiles(event)) return;
      event.preventDefault();
      if (event.dataTransfer) event.dataTransfer.dropEffect = "copy";
    });
    window.addEventListener("dragleave", (event) => {
      if (!dragCarriesFiles(event)) return;
      dragDepth = Math.max(0, dragDepth - 1);
      if (dragDepth === 0) setDragging(false);
    });
    window.addEventListener("drop", (event) => {
      if (!dragCarriesFiles(event)) return;
      event.preventDefault();
      setDragging(false);
      void importDrop(event.dataTransfer).catch(reportFailure);
    });
    elements.filePicker.addEventListener("change", () => {
      const chosen = [...(elements.filePicker.files ?? [])].map(
        (file): DroppedFile => ({
          path: file.webkitRelativePath || file.name,
          file,
        }),
      );
      void loadFiles(chosen).catch(reportFailure);
    });
  }

  return { attach, setControls };
}
