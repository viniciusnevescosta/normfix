import { mount } from "svelte";
import Diagnostics from "./components/Diagnostics.svelte";
import FileTree from "./components/FileTree.svelte";
import ResultSummary from "./components/ResultSummary.svelte";
import { loadElements, requiredElement } from "./dom";
import { downloadProject, downloadSource } from "./downloads";
import { createSourceEditor, type SourceEditor } from "./editor";
import {
  type BrowserFileResult,
  type BrowserSummary,
  FormatterResponseError,
  parseFormatterResponse,
} from "./formatter-response";
import {
  type Locale,
  type MessageKey,
  SUPPORTED_LOCALES,
  translate,
  translatePlural,
} from "./i18n";
import { setLocale } from "./i18n-state.svelte";
import {
  chooseIdentity,
  hasStoredIdentity,
  loadBrowserIdentity,
  removeStoredIdentity,
} from "./identity";
import { localizeDocument, readInitialLocale, storeLocale } from "./localization";
import { mountStaticUi } from "./mount-ui";
import { type OfflineState, type OfflineSupport, startOfflineSupport } from "./offline/pwa";
import { ZipArchiveError } from "./project/archive";
import { openDraftRow } from "./project/draft-row";
import { collectDroppedFiles, type DroppedFile } from "./project/drop";
import {
  ImportBatchError,
  MAX_FILE_BYTES,
  MAX_FILES,
  MAX_PROJECT_BYTES,
  portablePathKey,
  readImportBatch,
  sourcePathProblem,
} from "./project/files";
import { ImportPlanError, planImport } from "./project/import-plan";
import { markersFor } from "./project/markers";
import { deserializeProject, isSameProject, serializeProject } from "./project/persistence";
import { movedPath, renamedPath, rewritePrefix, wouldContainItself } from "./project/tree";
import {
  type Appearance,
  applyThemePreference,
  isThemePreference,
  readStoredThemePreference,
  storeThemePreference,
  type ThemePreference,
  watchSystemAppearance,
} from "./theme";
import {
  chromeState,
  codeState,
  confirmState,
  diagnosticsState,
  dragState,
  editorState,
  headerState,
  identityState,
  resultState,
  statusState,
  treeState,
} from "./tree-state.svelte";

const UTF8_ENCODER = new TextEncoder();
const PROJECT_STORAGE_KEY = "normfix.project.v1";

const SAMPLE: string = `#include <unistd.h>

int main(void)
{
    if (write(1, "normfix\\n", 8) > 0) { return (0); }
    else { return (1); }
}
`;

type RuntimeState = "loading" | "ready" | "error";
interface ResultRecord extends BrowserFileResult {
  inputSource: string;
}

type FormatProject = (request: string) => string;

interface WasmModule {
  default: () => Promise<unknown>;
  formatProject: FormatProject;
}

interface AppState {
  files: Map<string, string>;
  selected: string | null;
  results: Map<string, ResultRecord>;
  selectedResult: string | null;
  formatter: FormatProject | null;
  editor: SourceEditor | null;
  running: boolean;
  importing: boolean;
  revision: number;
  identityEmail: string | null;
  locale: Locale;
  offlineState: OfflineState;
  offlineSupport: OfflineSupport | null;
  theme: ThemePreference;
  appearance: Appearance;
  /** Paths the project holds but normfix does not format. */
  unsupported: Set<string>;
}

const state: AppState = {
  files: new Map<string, string>([["main.c", SAMPLE]]),
  selected: "main.c",
  results: new Map<string, ResultRecord>(),
  selectedResult: null,
  formatter: null,
  editor: null,
  running: false,
  importing: false,
  revision: 0,
  identityEmail: null,
  locale: readInitialLocale(),
  offlineState: "unsupported",
  offlineSupport: null,
  theme: readStoredThemePreference(),
  appearance: "dark",
  unsupported: new Set<string>(),
};

const elements = loadElements();

function t(key: MessageKey, values: Readonly<Record<string, string | number>> = {}): string {
  return translate(state.locale, key, values);
}

function tPlural(base: string, count: number): string {
  return translatePlural(state.locale, base, count);
}

function loadIdentity(): void {
  state.identityEmail = loadBrowserIdentity();
  renderIdentityControls();
}

/**
 * Pushes the identity panel what it draws.
 *
 * The panel derives the box and the buttons from `stored`, so there is no
 * combination where it offers to remember something already remembered.
 */
function renderIdentityControls(): void {
  identityState.stored = hasStoredIdentity();
  identityState.email = state.identityEmail ?? "";
}

function setIdentityStatus(key: MessageKey, invalid = false): void {
  identityState.status = t(key);
  identityState.invalid = invalid;
}

function saveIdentity(typed: string, remember: boolean): void {
  const choice = chooseIdentity(typed, remember);
  if (choice.email === null) {
    setIdentityStatus("invalidIdentity", true);
    return;
  }
  state.identityEmail = choice.email;
  state.revision += 1;
  invalidateResults();
  setIdentityStatus(
    choice.outcome === "saved"
      ? "identitySaved"
      : choice.outcome === "storage_unavailable"
        ? "storageUnavailable"
        : "identitySession",
  );
  renderIdentityControls();
}

function forgetIdentity(): void {
  state.identityEmail = null;
  state.revision += 1;
  invalidateResults();
  removeStoredIdentity();
  setIdentityStatus("identityForgotten");
  renderIdentityControls();
}

function applyTranslations(): void {
  chromeState.locale = state.locale;
  chromeState.theme = state.theme;
  // The runtime badge is the one message the page writes rather than marks up,
  // so it is re-said from the key it was set with instead of from an attribute
  // that no longer exists.
  if (runtimeMessageKey) statusState.runtimeLabel = t(runtimeMessageKey);
  renderOfflineStatus();
  elements.addFile.setAttribute("aria-label", t("addFile"));
  elements.addFolder.setAttribute("aria-label", t("addFolder"));
  chromeState.docsHref = localizeDocument(state.locale, elements, (key) => t(key)).docsHref;
  updateEditorMeta();
  renderOfflineStatus();
  if (state.results.size > 0) renderRunResult();
  else resetCopyLabel();
}

function changeLocale(locale: Locale): void {
  state.locale = locale;
  // The components read the language rather than being handed a function, so
  // this is what re-says every word they have already drawn.
  setLocale(locale);
  storeLocale(locale);
  applyTranslations();
}

function setRuntime(stateName: RuntimeState, label: string): void {
  statusState.runtime = stateName;
  statusState.runtimeLabel = label;
  runtimeMessageKey = null;
}

/** The message key the badge is showing, so a language change can re-say it. */
let runtimeMessageKey: MessageKey | null = null;

function setRuntimeMessage(stateName: RuntimeState, key: MessageKey): void {
  setRuntime(stateName, t(key));
  runtimeMessageKey = key;
}

/**
 * The badge says nothing while offline support is simply working, because that
 * is the normal state of an installed playground. It speaks for the two events
 * that change what the reader can do.
 */
function renderOfflineStatus(): void {
  statusState.offline = state.offlineState;
}

async function loadFormatter(): Promise<void> {
  try {
    const module = (await import("../pkg/normfix_wasm.js")) as WasmModule;
    await module.default();
    state.formatter = module.formatProject;
    refreshRunControl();
    setRuntimeMessage("ready", "wasmReady");
  } catch (error) {
    console.error(error);
    setRuntimeMessage("error", "wasmRequired");
    refreshRunControl();
    elements.run.dataset.i18nTitle = "wasmBuildHelp";
    elements.run.title = t("wasmBuildHelp");
  }
}

function refreshRunControl(): void {
  elements.run.disabled =
    state.formatter === null ||
    state.editor === null ||
    state.running ||
    state.importing ||
    state.files.size === 0;
}

function invalidateResults(): void {
  scheduleSave();
  state.results.clear();
  state.selectedResult = null;
  elements.results.hidden = true;
  // The panel is hidden, but an enabled control with nothing to act on is the
  // kind of state that only stays harmless by accident.
  resultState.usable = false;
  resultState.applicable = 0;
  // Marks describe a result. Once there is no result they describe a file that
  // may already have been edited past them.
  paintMarkers();
}

function syncEditor(): void {
  if (!state.editor) return;
  const value = state.editor.getValue();
  if (state.selected && state.files.get(state.selected) !== value) {
    state.files.set(state.selected, value);
    state.revision += 1;
    invalidateResults();
  }
}

function selectFile(path: string, syncCurrent = true): void {
  if (syncCurrent) syncEditor();
  enableEditor();
  const source = state.files.get(path);
  if (source === undefined) return;
  state.selected = path;
  state.editor?.setFile(path, source);
  headerState.path = path;
  updateEditorMeta();
  renderFileList();
}

/** Opens a draft row wired to this page's project state. */
function openDraft(kind: "file" | "folder"): void {
  if (state.importing) return;
  openDraftRow({
    container: elements.fileList,
    kind,
    label: t(kind === "file" ? "addFile" : "addFolder"),
    create: (path) => addSource(path, ""),
    onClose: () => state.editor?.focus(),
  });
}

/**
 * Applies a set of path rewrites as one change.
 *
 * All or nothing: a move that would collide with an existing name, or produce a
 * path the project cannot store, leaves the project exactly as it was. Half a
 * rename is a project the reader did not ask for and cannot undo.
 */
function applyMoves(moves: Array<readonly [string, string]>): void {
  if (state.importing || moves.length === 0) return;
  syncEditor();
  const proposed = new Map(state.files);
  const proposedUnsupported = new Set(state.unsupported);
  for (const [from] of moves) {
    proposed.delete(from);
    proposedUnsupported.delete(from);
  }
  for (const [from, to] of moves) {
    const supported = state.files.has(from);
    const problem = sourcePathProblem(to);
    const normalized = supported
      ? normalizeSourcePath(to)
      : problem?.code === "only_supported"
        ? to
        : (() => {
            if (problem?.code === "path_bytes") {
              throw new Error(t("pathBytes", { count: problem.count }));
            }
            throw new Error(t(problem === null ? "unsupportedFile" : "portablePath"));
          })();
    const clashes = [...proposed.keys(), ...proposedUnsupported].some(
      (loaded) => portablePathKey(loaded) === portablePathKey(normalized),
    );
    if (clashes) throw new Error(t("importConflict", { path: normalized }));
    if (supported) {
      const source = state.files.get(from);
      if (source === undefined) throw new Error(t("responsePath", { path: from }));
      proposed.set(normalized, source);
    } else {
      proposedUnsupported.add(normalized);
    }
  }
  if (proposed.size > 0) validateProjectSources(proposed);
  const selected = moves.find(([from]) => from === state.selected);
  const shownUnsupported = moves.find(
    ([from]) => from === headerState.path && state.selected === null,
  );
  for (const [from] of moves) {
    if (state.files.has(from)) state.editor?.removeFile(from);
  }
  state.files = proposed;
  state.unsupported = proposedUnsupported;
  state.revision += 1;
  invalidateResults();
  const reselect = selected ? normalizeSourcePath(selected[1]) : state.selected;
  if (shownUnsupported) showUnsupported(shownUnsupported[1]);
  else if (reselect === null) renderFileList();
  else selectFile(reselect, false);
}

/** Moves one entry, or a whole folder, under `folder`. */
function moveEntry(path: string, isFolder: boolean, folder: string): void {
  if (isFolder && wouldContainItself(path, folder)) return;
  const paths = [...state.files.keys(), ...state.unsupported];
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

/** Renames one entry's last segment, carrying a folder's contents with it. */
function renameEntry(path: string, isFolder: boolean, name: string): void {
  const target = renamedPath(path, name);
  if (target === path) return;
  try {
    applyMoves(
      isFolder
        ? rewritePrefix([...state.files.keys(), ...state.unsupported], path, target)
        : [[path, target]],
    );
  } catch (failure) {
    reportProjectError(failure);
  }
}

/** Removes one entry, or a folder and everything under it. */
function deleteEntry(path: string, isFolder: boolean): void {
  if (state.importing) return;
  syncEditor();
  const under = (loaded: string): boolean =>
    loaded === path || (isFolder && loaded.startsWith(`${path}/`));
  const removed = [...state.files.keys()].filter(under);
  // A file the project shows but cannot format lives beside the formattable
  // ones, not among them. Deleting only from `files` left it in the tree with
  // no way to remove it — visible, permanent, and not part of the project.
  const dropped = [...state.unsupported].filter(under);
  if (removed.length === 0 && dropped.length === 0) return;
  const proposed = new Map(state.files);
  for (const loaded of removed) {
    proposed.delete(loaded);
    state.editor?.removeFile(loaded);
  }
  state.files = proposed;
  for (const loaded of dropped) state.unsupported.delete(loaded);
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

/**
 * Shows a project with nothing in it.
 *
 * Deleting the last file used to be refused, which read as the page being
 * broken rather than as a rule. It is allowed now, and what follows says so:
 * the editor is off, and the notice invites the reader to create or import
 * instead of leaving an empty text box that would accept typing nobody could
 * format.
 */
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

let treeMounted = false;

/**
 * Hands the panel what to draw.
 *
 * This used to rebuild the list by hand, and every change had to remember to
 * call it. What it now updates is state the panel reads, so a field left stale
 * shows a stale row rather than a panel describing a project that is gone.
 */
function renderFileList(): void {
  treeState.files = [...state.files.keys(), ...state.unsupported];
  treeState.unsupported = new Set(state.unsupported);
  treeState.changed = new Set(
    [...state.results.entries()].filter(([, result]) => result.changed).map(([path]) => path),
  );
  treeState.selected = state.selected;
  if (treeMounted) return;
  treeMounted = true;
  mount(FileTree, {
    target: elements.fileList,
    props: {
      get files() {
        return treeState.files;
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
      translate: (key: string) => t(key as MessageKey),
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

/**
 * Shows a file the project holds but normfix cannot format.
 *
 * The editor is turned off rather than opened empty or opened read-only with
 * no explanation: a text box that will not do anything, with nothing saying
 * why, reads as the page being broken.
 */
function showUnsupported(path: string): void {
  syncEditor();
  state.selected = null;
  headerState.path = path;
  editorState.notice = { title: t("unsupportedFile"), detail: t("supportedKinds") };
  refreshRunControl();
  renderFileList();
}

/** Turns the editor back on for a file normfix does format. */
function enableEditor(): void {
  editorState.notice = null;
  refreshRunControl();
}

/**
 * Asks before deleting, because there is no undo in this tab.
 *
 * A folder names how many files go with it: `src` is one row on screen and can
 * be a dozen files, and the count is the part the reader cannot see.
 */
function confirmDelete(path: string, isFolder: boolean): void {
  const count = isFolder
    ? [...state.files.keys()].filter((loaded) => loaded.startsWith(`${path}/`)).length
    : 1;
  confirmState.request = {
    text: isFolder
      ? t("deleteFolderText", { path, count: String(count) })
      : t("deleteFileText", { path }),
  };
  confirmState.accept = () => deleteEntry(path, isFolder);
}

function updateEditorMeta(): void {
  const source = state.editor?.getValue() ?? state.files.get(state.selected ?? "") ?? "";
  headerState.lines = source.length === 0 ? 0 : source.split("\n").length;
  headerState.bytes = UTF8_ENCODER.encode(source).length;
}

function fileKind(path: string): string {
  const filename = path.split("/").at(-1)?.toLowerCase() ?? "";
  if (filename === "makefile") return "MK";
  if (filename.endsWith(".md")) return "MD";
  if (filename.endsWith(".h")) return "H";
  return "C";
}

function normalizeSourcePath(path: string): string {
  const problem = sourcePathProblem(path);
  if (!problem) return path;
  if (problem.code === "only_supported") throw new Error(t("onlySupported"));
  if (problem.code === "path_bytes") {
    throw new Error(t("pathBytes", { count: problem.count }));
  }
  throw new Error(t("portablePath"));
}

function validateProjectSources(files: ReadonlyMap<string, string>): void {
  if (files.size === 0) throw new Error(t("emptyProject"));
  if (files.size > MAX_FILES) {
    throw new Error(t("maxFiles", { count: MAX_FILES }));
  }
  let projectBytes = 0;
  const portablePaths = new Set<string>();
  for (const [path, source] of files) {
    const normalized = normalizeSourcePath(path);
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

function addSource(path: string, source = ""): void {
  if (state.importing) throw new Error(t("waitForImport"));
  syncEditor();
  const normalized = normalizeSourcePath(path);
  if (
    [...state.files.keys()].some(
      (loaded) => portablePathKey(loaded) === portablePathKey(normalized),
    )
  ) {
    throw new Error(t("importConflict", { path: normalized }));
  }
  const proposed = new Map(state.files);
  proposed.set(normalized, source);
  validateProjectSources(proposed);
  state.files = proposed;
  state.revision += 1;
  invalidateResults();
  selectFile(normalized, false);
}

function removeSelected(): void {
  if (state.importing || !state.selected) return;
  confirmDelete(state.selected, false);
}

/**
 * Imports files, skipping what normfix does not format rather than refusing
 * the whole batch.
 *
 * A student drops a project folder, which contains object files, a binary, and
 * a README beside the source. Failing the import over those would make the
 * feature useless for its main use, so they are counted and reported instead.
 * Nothing is skipped quietly: the count is always shown, and when nothing at
 * all could be imported the first rejected path explains exactly why.
 */
async function loadFiles(
  incoming: readonly DroppedFile[],
  unsupported: readonly string[] = [],
  alreadySkipped = 0,
): Promise<void> {
  if (incoming.length === 0 && unsupported.length === 0 && alreadySkipped === 0) return;
  if (state.importing) throw new Error(t("importRunning"));
  syncEditor();
  const startingRevision = state.revision;
  state.importing = true;
  setImportControls(true);
  try {
    let plan: ReturnType<typeof planImport>;
    try {
      plan = planImport(
        incoming,
        unsupported,
        state.files.keys(),
        state.unsupported,
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
    if (candidates.size === 0 && !unsupportedChanged) {
      if (plan.firstRejected !== null) normalizeSourcePath(plan.firstRejected);
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
      imported = await readImportBatch(candidates.values(), startingRevision, () => state.revision);
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
    validateProjectSources(proposed);
    if (state.revision !== startingRevision) {
      throw new Error(t("importChanged"));
    }
    state.files = proposed;
    state.unsupported = plan.unsupported;
    state.revision += 1;
    invalidateResults();
    if (imported.selectedPath) selectFile(imported.selectedPath, false);
    renderFileList();
    const messages: string[] = [];
    if (candidates.size > 0) messages.push(tPlural("imported", candidates.size));
    if (plan.ignored > 0) messages.push(tPlural("skipped", plan.ignored));
    setRuntime("ready", messages.join(" "));
  } finally {
    state.importing = false;
    setImportControls(false);
    elements.filePicker.value = "";
  }
}

function setImportControls(disabled: boolean): void {
  elements.filePicker.disabled = disabled;
  elements.addFile.disabled = disabled;
  elements.addFolder.disabled = disabled;
  elements.removeFile.disabled = disabled;
  refreshRunControl();
}

async function runFormatter(): Promise<void> {
  if (
    !state.formatter ||
    !state.editor ||
    state.running ||
    state.importing ||
    state.files.size === 0
  ) {
    return;
  }
  syncEditor();
  state.running = true;
  refreshRunControl();
  setRuntimeMessage("loading", "formatting");
  try {
    // Yield once so the "formatting" message paints before the synchronous
    // WebAssembly call takes the thread. An animation frame alone would be a
    // trap: a hidden or backgrounded tab never fires one, so a reader who
    // pressed Run and switched away came back to a disabled button and a
    // message that had been waiting the whole time. The timer always fires.
    await new Promise<void>((resolve) => {
      let done = false;
      const settle = (): void => {
        if (done) return;
        done = true;
        resolve();
      };
      requestAnimationFrame(settle);
      window.setTimeout(settle, 50);
    });
    validateProjectSources(state.files);
    const request = {
      files: [...state.files.entries()].map(([path, source]) => ({ path, source })),
      identity_email: state.identityEmail,
      timestamp: formatHeaderTimestamp(new Date()),
    };
    const inputSources = new Map(request.files.map((file) => [file.path, file.source]));
    let response: ReturnType<typeof parseFormatterResponse>;
    try {
      response = parseFormatterResponse(state.formatter(JSON.stringify(request)), inputSources);
    } catch (error) {
      if (error instanceof FormatterResponseError && error.code === "path") {
        throw new Error(t("responsePath", { path: error.path ?? "?" }));
      }
      if (error instanceof FormatterResponseError) throw new Error(t("responseSchema"));
      throw error;
    }
    state.results = new Map(
      response.files.map((file): [string, ResultRecord] => {
        const inputSource = inputSources.get(file.path)!;
        return [file.path, { ...file, inputSource }];
      }),
    );
    state.selectedResult =
      state.selected && state.results.has(state.selected)
        ? state.selected
        : (response.files[0]?.path ?? null);
    renderRunResult(response.summary);
    paintMarkers();
    setRuntimeMessage("ready", "wasmReady");
    elements.results.hidden = false;
    elements.results.scrollIntoView({
      behavior: matchMedia("(prefers-reduced-motion: reduce)").matches ? "auto" : "smooth",
      block: "start",
    });
  } catch (error) {
    console.error(error);
    setRuntime("error", error instanceof Error ? error.message : String(error));
  } finally {
    state.running = false;
    refreshRunControl();
  }
}

function formatHeaderTimestamp(date: Date): string {
  const part = (value: number): string => String(value).padStart(2, "0");
  return `${date.getFullYear()}/${part(date.getMonth() + 1)}/${part(date.getDate())} ${part(date.getHours())}:${part(date.getMinutes())}:${part(date.getSeconds())}`;
}

/// Underlines each file's findings where they are, the way an editor does.
///
/// A diagnostic list beside the code makes a reader match a line number by eye,
/// which is exactly the work an editor exists to remove. The findings describe
/// the file that was submitted, so the marks belong on the input rather than on
/// the formatted output — and they are cleared for a file that came back clean,
/// otherwise a fixed file keeps wearing the marks of its previous run.
function paintMarkers(): void {
  const editor = state.editor;
  if (!editor?.usingMonaco) return;
  for (const path of state.files.keys()) {
    const result = state.results.get(path);
    editor.setMarkers(path, markersFor(result?.diagnostics ?? []));
  }
}

function renderRunResult(providedSummary: BrowserSummary | null = null): void {
  if (state.results.size === 0) {
    elements.results.hidden = true;
    return;
  }
  const files = [...state.results.values()];
  const summary = providedSummary ?? {
    files: files.length,
    changed: files.filter((file) => file.changed).length,
    fixes: files.reduce((total, file) => total + file.fixes.length, 0),
    diagnostics: files.reduce((total, file) => total + file.diagnostics.length, 0),
    failed: files.filter((file) => file.error).length,
  };
  renderSummary(summary);
  if (!state.selectedResult || !state.results.has(state.selectedResult)) {
    state.selectedResult = files[0]?.path ?? null;
  }
  renderSelectedResult();
  renderFileList();
  elements.results.hidden = false;
}

function renderSummary(summary: BrowserSummary): void {
  resultState.summary = summary;
}

/**
 * Hands the result header everything it draws.
 *
 * The counts, the picker, the five buttons and the tabs were disabled and
 * re-enabled by hand after every run, apply and edit. They are derived from
 * the shown result now, so a button that would do nothing cannot be left
 * clickable.
 */
function renderResultHeader(): void {
  const result = selectedResult();
  const usable = result !== undefined && !result.error && result.stable;
  resultState.paths = [...state.results.keys()];
  resultState.selected = state.selectedResult ?? "";
  resultState.usable = usable;
  resultState.applicable = applicableResults().length;
  resultState.diagnosticCount = result?.diagnostics.length ?? 0;
  if (resultMounted) return;
  resultMounted = true;
  mount(ResultSummary, {
    target: elements.resultSummary,
    props: {
      get summary() {
        return resultState.summary;
      },
      get paths() {
        return resultState.paths;
      },
      get selected() {
        return resultState.selected;
      },
      get usable() {
        return resultState.usable;
      },
      get applicable() {
        return resultState.applicable;
      },
      get diagnosticCount() {
        return resultState.diagnosticCount;
      },
      get view() {
        return resultState.view;
      },
      get copyLabel() {
        return resultState.copyLabel;
      },
      translate: (key: string) => t(key as MessageKey),
      onSelect: (path: string) => {
        state.selectedResult = path;
        renderSelectedResult();
      },
      onView: activateTab,
      onApply: applySelectedResult,
      onApplyAll: applyAllResults,
      onCopy: () => {
        void copyCurrent();
      },
      onDownload: downloadCurrent,
      onDownloadAll: downloadAll,
    },
  });
}

let resultMounted = false;

function selectedResult(): ResultRecord | undefined {
  if (!state.selectedResult) return undefined;
  const result = state.results.get(state.selectedResult);
  if (!result || state.files.get(result.path) !== result.inputSource) return undefined;
  return result;
}

function renderSelectedResult(): void {
  const result = selectedResult();
  if (!result) return;
  codeState.formatted = result.formatted;
  codeState.diff = result.diff || t("noByteChanges");
  resetCopyLabel();
  renderResultHeader();
  renderDiagnostics(result);
}

/**
 * Hands the diagnostics panel one file's result.
 *
 * This used to build every card, list and table by hand, and returned early on
 * an unreadable file — which is how the one finding worth acting on ended up
 * hidden behind the reason it was not written.
 */
function renderDiagnostics(result: ResultRecord): void {
  diagnosticsState.diagnostics = result.diagnostics;
  diagnosticsState.fixes = result.fixes;
  diagnosticsState.budget = result.budget;
  diagnosticsState.error = result.error;
  diagnosticsState.stable = result.stable;
  if (diagnosticsMounted) return;
  diagnosticsMounted = true;
  mount(Diagnostics, {
    target: elements.diagnosticsView,
    props: {
      get diagnostics() {
        return diagnosticsState.diagnostics;
      },
      get fixes() {
        return diagnosticsState.fixes;
      },
      get budget() {
        return diagnosticsState.budget;
      },
      get error() {
        return diagnosticsState.error;
      },
      get stable() {
        return diagnosticsState.stable;
      },
      translate: (key: string, values?: Record<string, string | number>) =>
        values ? t(key as MessageKey, values as Record<string, string>) : t(key as MessageKey),
    },
  });
}

let diagnosticsMounted = false;

function activateTab(view: "formatted" | "diagnostics" | "diff"): void {
  resultState.view = view;
  for (const name of ["formatted", "diagnostics", "diff"] as const) {
    requiredElement<HTMLElement>(`#${name}-view`).hidden = name !== view;
  }
}

function applySelectedResult(): void {
  const result = selectedResult();
  if (!result || result.error || !result.stable) return;
  applyResults([result]);
  selectFile(result.path, false);
  state.editor?.focus();
}

/**
 * Every result that is still safe to apply.
 *
 * A result stops being applicable once its file has been edited since the run
 * that produced it, because the fix was proven against the source it read, not
 * against whatever is in the buffer now.
 */
function applicableResults(): ResultRecord[] {
  return [...state.results.values()].filter(
    (result) =>
      !result.error &&
      result.stable &&
      state.files.get(result.path) === result.inputSource &&
      result.formatted !== result.inputSource,
  );
}

function applyAllResults(): void {
  const results = applicableResults();
  if (results.length === 0) return;
  applyResults(results);
  if (state.selected) selectFile(state.selected, false);
  setRuntime("ready", tPlural("fixed", results.length));
  state.editor?.focus();
}

function applyResults(results: readonly ResultRecord[]): void {
  for (const result of results) state.files.set(result.path, result.formatted);
  state.revision += 1;
  invalidateResults();
  renderFileList();
}

let copyLabelTimer: number | undefined;

function resetCopyLabel(): void {
  if (copyLabelTimer !== undefined) {
    window.clearTimeout(copyLabelTimer);
    copyLabelTimer = undefined;
  }
  resultState.copyLabel = t("copyFile");
}

function flashCopyLabel(label: string): void {
  resultState.copyLabel = label;
  if (copyLabelTimer !== undefined) window.clearTimeout(copyLabelTimer);
  copyLabelTimer = window.setTimeout(resetCopyLabel, 1600);
}

/// Selects the formatted output so a keyboard copy works.
function selectFormattedOutput(): void {
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  if (!formattedElement) return;
  range.selectNodeContents(formattedElement);
  selection.removeAllRanges();
  selection.addRange(range);
}

/// Copies the formatted source of the selected file to the clipboard.
async function copyCurrent(): Promise<void> {
  const result = selectedResult();
  if (!result || result.error || !result.stable) return;
  try {
    await navigator.clipboard.writeText(result.formatted);
    flashCopyLabel(t("copied"));
  } catch {
    // The clipboard needs a secure context and a trusted user gesture, and a
    // browser can refuse for either reason. Select the text so the keyboard
    // shortcut still works instead of leaving the reader with nothing.
    selectFormattedOutput();
    flashCopyLabel(t("pressCopy"));
  }
}

function downloadCurrent(): void {
  const result = selectedResult();
  if (!result || result.error || !result.stable) return;
  downloadSource(result.path, result.formatted);
}

function downloadAll(): void {
  const files = [...state.results.values()]
    .filter((file) => !file.error && file.stable && state.files.get(file.path) === file.inputSource)
    .map((file) => ({ path: file.path, source: file.formatted }));
  if (files.length === 0) return;
  try {
    downloadProject(files);
  } catch (error) {
    if (error instanceof ZipArchiveError) {
      setRuntime("error", t("archivePath", { path: error.path }));
      return;
    }
    throw error;
  }
}

elements.run.addEventListener("click", runFormatter);
/**
 * Whether a drag is carrying files rather than text.
 *
 * Monaco drags selected text within the editor, and that must keep working.
 */
function dragCarriesFiles(event: DragEvent): boolean {
  return [...(event.dataTransfer?.types ?? [])].includes("Files");
}

// dragenter and dragleave both fire while the pointer crosses child elements,
// so the overlay is driven by a depth counter rather than by the last event.
let dragDepth = 0;

function setDragging(active: boolean): void {
  dragDepth = active ? dragDepth : 0;
  dragState.active = active;
}

window.addEventListener("dragenter", (event) => {
  if (!dragCarriesFiles(event)) return;
  event.preventDefault();
  dragDepth += 1;
  dragState.active = true;
});

window.addEventListener("dragover", (event) => {
  if (!dragCarriesFiles(event)) return;
  // Without this the browser navigates away to the dropped file, discarding
  // whatever the student had in the editor.
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
  void importDrop(event.dataTransfer).catch((error: unknown) => {
    setRuntime("error", error instanceof Error ? error.message : String(error));
  });
});

async function importDrop(transfer: DataTransfer | null): Promise<void> {
  if (!transfer) return;
  // The entries must be taken before this function first yields: the browser
  // empties DataTransfer.items as soon as the drop handler returns.
  const entries = [...transfer.items]
    .map((item) => item.webkitGetAsEntry())
    .filter((entry): entry is FileSystemEntry => entry !== null);
  if (entries.length === 0) {
    // No entry API, or a drop that carried plain files only. Folders are not
    // reachable this way, but individual files still are.
    await loadFiles([...transfer.files].map((file) => ({ path: file.name, file })));
    return;
  }
  const selection = await collectDroppedFiles(entries);
  await loadFiles(selection.files, selection.unsupported, selection.skipped);
}

elements.filePicker.addEventListener("change", () => {
  const chosen = [...(elements.filePicker.files ?? [])].map(
    (file): DroppedFile => ({
      path: file.webkitRelativePath || file.name,
      file,
    }),
  );
  void loadFiles(chosen).catch((error: unknown) => {
    setRuntime("error", error instanceof Error ? error.message : String(error));
  });
});
elements.removeFile.addEventListener("click", removeSelected);
function startRename(path: string, isFolder: boolean): void {
  // The row belongs to the panel now, and is found by the path it carries
  // rather than passed in by whoever opened the menu.
  const entry = elements.fileList.querySelector<HTMLElement>(`[data-path="${CSS.escape(path)}"]`);
  const label = entry?.querySelector<HTMLElement>("span:nth-child(2)");
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

/**
 * Writes the project to this browser, so closing the tab does not cost it.
 *
 * Debounced, because this runs on every keystroke through the editor's sync,
 * and a project of a few files is a few hundred kilobytes to re-encode. A
 * write that fails — storage full, storage denied — is dropped silently: the
 * work is still on screen, and a warning about a cache nobody asked for would
 * interrupt a reader for something they cannot act on.
 */
let saveTimer: ReturnType<typeof setTimeout> | null = null;

function saveProject(): void {
  const payload = serializeProject({
    files: Object.fromEntries(state.files),
    selected: state.selected,
    unsupported: [...state.unsupported],
    savedAt: Date.now(),
  });
  try {
    if (payload === null) localStorage.removeItem(PROJECT_STORAGE_KEY);
    else localStorage.setItem(PROJECT_STORAGE_KEY, payload);
  } catch {
    // The project is still open; nothing here is worth interrupting for.
  }
}

function scheduleSave(): void {
  if (saveTimer !== null) clearTimeout(saveTimer);
  saveTimer = setTimeout(saveProject, 600);
}

/**
 * Brings back the project this browser was holding.
 *
 * Never silently: this page is built for campus machines, which are shared,
 * and code restored without a word is code the next person at that keyboard
 * did not expect to find. The banner says the work came back and offers to
 * drop it, which is also the only way to clear what is stored.
 */
function restoreProject(): void {
  const stored = deserializeProject(readStoredProject());
  if (!stored || isSameProject(stored, state.files)) return;
  state.files = new Map(Object.entries(stored.files));
  state.unsupported = new Set(stored.unsupported);
  state.revision += 1;
  const selected =
    stored.selected !== null && state.files.has(stored.selected)
      ? stored.selected
      : [...state.files.keys()].sort()[0];
  // The editor is created after restore. Publishing the tree before that
  // would let a fast click select a second file while Monaco was still loading
  // the first, leaving state and model on different paths.
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

function readStoredProject(): string | null {
  try {
    return localStorage.getItem(PROJECT_STORAGE_KEY);
  } catch {
    return null;
  }
}

function discardStoredProject(): void {
  try {
    localStorage.removeItem(PROJECT_STORAGE_KEY);
  } catch {
    // Nothing stored is the state being asked for either way.
  }
  state.files = new Map([["main.c", SAMPLE]]);
  state.unsupported = new Set();
  state.revision += 1;
  invalidateResults();
  selectFile("main.c", false);
  elements.restoreNotice.hidden = true;
}

elements.discardRestore.addEventListener("click", discardStoredProject);

// A reload fires `pagehide`; closing a tab on mobile often only fires
// `visibilitychange`. Both write, because the debounce may not have run.
addEventListener("pagehide", saveProject);
addEventListener("visibilitychange", () => {
  if (document.visibilityState === "hidden") saveProject();
});

elements.addFile.addEventListener("click", () => openDraft("file"));
elements.addFolder.addEventListener("click", () => openDraft("folder"));
/**
 * Repaints for the chosen appearance.
 *
 * Like the language selector, this changes how the page looks and nothing
 * else: no run, no request, no reload.
 */
function changeTheme(preference: ThemePreference): void {
  state.theme = preference;
  state.appearance = applyThemePreference(preference);
  state.editor?.setAppearance(state.appearance);
  storeThemePreference(preference);
  chromeState.theme = preference;
}

// Only meaningful while following the system, but harmless otherwise: the
// preference is what decides, and re-applying it is idempotent.
watchSystemAppearance(() => {
  if (state.theme === "system") changeTheme("system");
});

/** The element holding the formatted text, for selecting it to copy. */
let formattedElement: HTMLElement | null = null;

mountStaticUi({
  elements,
  locale: () => state.locale,
  translate: t,
  onLocale: (locale) => {
    if (SUPPORTED_LOCALES.includes(locale as Locale)) changeLocale(locale as Locale);
  },
  onTheme: (value) => {
    if (isThemePreference(value)) changeTheme(value);
  },
  onSaveIdentity: saveIdentity,
  onForgetIdentity: forgetIdentity,
  onOfflineUpdate: () => state.offlineSupport?.applyUpdate(),
  onFormattedElement: (element) => {
    formattedElement = element;
  },
});

async function initialize(): Promise<void> {
  // The static buttons exist before the asynchronously loaded editor does.
  // Keep them inert until there is a model to receive their state changes.
  setImportControls(true);
  setLocale(state.locale);
  // The old markup carried this as text; the badge is written from state now.
  setRuntimeMessage("loading", "loadingFormatter");
  state.appearance = applyThemePreference(state.theme);
  applyTranslations();
  loadIdentity();
  // Before the first render, so the panel never shows the sample project for a
  // frame and then replaces it with the reader's own.
  restoreProject();
  state.offlineSupport = startOfflineSupport({
    onState: (offlineState) => {
      state.offlineState = offlineState;
      renderOfflineStatus();
    },
    onConnectivity: (online) => {
      statusState.online = online;
    },
  });
  const formatterPromise = loadFormatter();
  // The editor opens on whatever the project ended up holding, which is the
  // restored file when there was one. Opening on the sample and correcting it
  // afterwards would show the reader a file that is not theirs first.
  const opening = state.selected ?? "main.c";
  // The old markup carried the first filename as text, so nothing had to say
  // it. The header is written from state now, and the first file is the one
  // case nothing else announces.
  headerState.path = opening;
  try {
    state.editor = await createSourceEditor(
      elements.editorContainer,
      elements.fallbackEditor,
      opening,
      state.files.get(opening) ?? SAMPLE,
      state.appearance,
      {
        onChange: () => {
          syncEditor();
          updateEditorMeta();
        },
        onRun: () => {
          void runFormatter();
        },
      },
    );
  } finally {
    setImportControls(false);
  }
  renderFileList();
  updateEditorMeta();
  await formatterPromise;
}

void initialize().catch((error: unknown) => {
  console.error(error);
  setRuntime("error", error instanceof Error ? error.message : String(error));
});
