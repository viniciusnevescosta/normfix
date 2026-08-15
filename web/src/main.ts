import { createSourceEditor, type SourceEditor } from "./editor";
import { startOfflineSupport, type OfflineState, type OfflineSupport } from "./offline/pwa";
import {
  applyThemePreference,
  isThemePreference,
  readStoredThemePreference,
  storeThemePreference,
  watchSystemAppearance,
  type Appearance,
  type ThemePreference,
} from "./theme";
import {
  SUPPORTED_LOCALES,
  detectLocale,
  translate,
  translatePlural,
  type Locale,
  type MessageKey,
} from "./i18n";
import {
  ImportBatchError,
  MAX_FILES,
  MAX_FILE_BYTES,
  MAX_PROJECT_BYTES,
  canonicalIdentityEmail,
  portablePathKey,
  readImportBatch,
  sourcePathProblem,
  type ProjectSourceFile,
} from "./project/files";
import { ZipArchiveError, buildZip } from "./project/archive";
import { markersFor } from "./project/markers";
import { collectDroppedFiles, type DroppedFile } from "./project/drop";
import { openDraftRow } from "./project/draft-row";
import {
  buildTree,
  movedPath,
  renamedPath,
  rewritePrefix,
  wouldContainItself,
  type TreeNode,
} from "./project/tree";
import { GITHUB_REPOSITORY_API, githubRequestInit, starCount } from "./github";

const UTF8_ENCODER = new TextEncoder();
const FOLDER_SHUT = "\u{1F4C1}";
const FOLDER_OPEN = "\u{1F4C2}";
const IDENTITY_STORAGE_KEY = "normfix.identity.v1";
const LOCALE_STORAGE_KEY = "normfix.locale.v1";
const FALLBACK_STARS = 0;

const SAMPLE: string = `#include <unistd.h>

int main(void)
{
    if (write(1, "normfix\\n", 8) > 0) { return (0); }
    else { return (1); }
}
`;

type RuntimeState = "loading" | "ready" | "error";
type Severity = "error" | "warning" | "info";

interface BrowserLocation {
  line: number;
  column: number;
}

interface BrowserFix {
  rule_id: string;
  description: string;
  line: number | null;
  applicability: string;
}

interface BrowserDiagnostic {
  rule_id: string;
  severity: Severity;
  message: string;
  location: BrowserLocation | null;
  help: string | null;
  notes: string[];
  source: string;
}

interface BrowserBudget {
  function: string;
  line: number;
  lines: number;
  line_limit: number;
  variables: number;
  variable_limit: number;
  parameters: number;
  parameter_limit: number;
}

interface BrowserFileResult {
  path: string;
  formatted: string;
  changed: boolean;
  stable: boolean;
  fixes: BrowserFix[];
  diagnostics: BrowserDiagnostic[];
  budget: BrowserBudget[];
  diff: string;
  error: string | null;
}

interface BrowserSummary {
  files: number;
  changed: number;
  fixes: number;
  diagnostics: number;
  failed: number;
}

interface PlaygroundResponse {
  schema_version: number;
  files: BrowserFileResult[];
  summary: BrowserSummary;
}

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
  /** Folders the reader has closed, by prefix. */
  collapsed: Set<string>;
  /** Paths the project holds but normfix does not format. */
  unsupported: Set<string>;
}

function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Required element is missing: ${selector}`);
  return element;
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
  locale: readStoredLocale(),
  offlineState: "unsupported",
  offlineSupport: null,
  theme: readStoredThemePreference(),
  appearance: "dark",
  collapsed: new Set<string>(),
  unsupported: new Set<string>(),
};

const elements = {
  runtime: requiredElement<HTMLElement>("#runtime-status"),
  runtimeLabel: requiredElement<HTMLElement>("#runtime-label"),
  fileList: requiredElement<HTMLElement>("#file-list"),
  filePicker: requiredElement<HTMLInputElement>("#file-picker"),
  dropOverlay: requiredElement<HTMLElement>("#drop-overlay"),
  addFile: requiredElement<HTMLButtonElement>("#add-file"),
  addFolder: requiredElement<HTMLButtonElement>("#add-folder"),
  removeFile: requiredElement<HTMLButtonElement>("#remove-file"),
  editorContainer: requiredElement<HTMLElement>("#monaco-editor"),
  fallbackEditor: requiredElement<HTMLTextAreaElement>("#fallback-editor"),
  editorTitle: requiredElement<HTMLElement>("#editor-title"),
  editorMeta: requiredElement<HTMLElement>("#editor-meta"),
  editorDisabled: requiredElement<HTMLElement>("#editor-disabled"),
  editorDisabledTitle: requiredElement<HTMLElement>("#editor-disabled-title"),
  editorDisabledText: requiredElement<HTMLElement>("#editor-disabled-text"),
  confirmDelete: requiredElement<HTMLDialogElement>("#confirm-delete"),
  confirmDeleteText: requiredElement<HTMLElement>("#confirm-delete-text"),
  confirmDeleteAction: requiredElement<HTMLButtonElement>("#confirm-delete-action"),
  confirmDeleteCancel: requiredElement<HTMLButtonElement>("#confirm-delete-cancel"),
  run: requiredElement<HTMLButtonElement>("#run"),
  results: requiredElement<HTMLElement>("#results"),
  summary: requiredElement<HTMLElement>("#summary"),
  resultFile: requiredElement<HTMLSelectElement>("#result-file"),
  applyResult: requiredElement<HTMLButtonElement>("#apply-result"),
  applyAll: requiredElement<HTMLButtonElement>("#apply-all"),
  copyCurrent: requiredElement<HTMLButtonElement>("#copy-current"),
  downloadCurrent: requiredElement<HTMLButtonElement>("#download-current"),
  downloadAll: requiredElement<HTMLButtonElement>("#download-all"),
  formattedOutput: requiredElement<HTMLElement>("#formatted-output"),
  diffOutput: requiredElement<HTMLElement>("#diff-output"),
  diagnosticsView: requiredElement<HTMLElement>("#diagnostics-view"),
  diagnosticCount: requiredElement<HTMLElement>("#diagnostic-count"),
  diagnosticTemplate: requiredElement<HTMLTemplateElement>("#diagnostic-template"),
  language: requiredElement<HTMLSelectElement>("#language"),
  theme: requiredElement<HTMLSelectElement>("#theme"),
  identityEmail: requiredElement<HTMLInputElement>("#identity-email"),
  rememberIdentity: requiredElement<HTMLInputElement>("#remember-identity"),
  rememberOption: requiredElement<HTMLElement>(".remember-option"),
  saveIdentity: requiredElement<HTMLButtonElement>("#save-identity"),
  forgetIdentity: requiredElement<HTMLButtonElement>("#forget-identity"),
  identityStatus: requiredElement<HTMLElement>("#identity-status"),
  starCount: requiredElement<HTMLElement>("#star-count"),
  offlineStatus: requiredElement<HTMLElement>("#offline-status"),
  offlineLabel: requiredElement<HTMLElement>("#offline-label"),
  offlineUpdate: requiredElement<HTMLButtonElement>("#offline-update"),
  docsLink: requiredElement<HTMLAnchorElement>("#docs-link"),
  brand: requiredElement<HTMLAnchorElement>(".brand"),
  canonical: requiredElement<HTMLLinkElement>("#canonical-url"),
  manifest: requiredElement<HTMLLinkElement>("#manifest-link"),
  metaDescription: requiredElement<HTMLMetaElement>("#meta-description"),
  ogTitle: requiredElement<HTMLMetaElement>("#og-title"),
  ogDescription: requiredElement<HTMLMetaElement>("#og-description"),
  ogUrl: requiredElement<HTMLMetaElement>("#og-url"),
  ogLocale: requiredElement<HTMLMetaElement>("#og-locale"),
  ogAlternates: [
    requiredElement<HTMLMetaElement>("#og-alternate-one"),
    requiredElement<HTMLMetaElement>("#og-alternate-two"),
    requiredElement<HTMLMetaElement>("#og-alternate-three"),
  ],
  twitterTitle: requiredElement<HTMLMetaElement>("#twitter-title"),
  twitterDescription: requiredElement<HTMLMetaElement>("#twitter-description"),
};

function t(key: MessageKey, values: Readonly<Record<string, string | number>> = {}): string {
  return translate(state.locale, key, values);
}

function tPlural(base: string, count: number): string {
  return translatePlural(state.locale, base, count);
}

function readStoredLocale(): Locale {
  const routeLocale = window.location.pathname.split("/").filter(Boolean)[0];
  if (routeLocale && SUPPORTED_LOCALES.includes(routeLocale as Locale)) {
    return routeLocale as Locale;
  }
  try {
    const stored = localStorage.getItem(LOCALE_STORAGE_KEY);
    if (SUPPORTED_LOCALES.includes(stored as Locale)) return stored as Locale;
  } catch {
    // A blocked storage API should not prevent the playground from starting.
  }
  return detectLocale();
}

function loadIdentity(): void {
  try {
    const stored = localStorage.getItem(IDENTITY_STORAGE_KEY);
    const canonical = stored ? canonicalIdentityEmail(stored) : null;
    if (canonical) {
      state.identityEmail = canonical;
      elements.identityEmail.value = canonical;
    } else if (stored) {
      localStorage.removeItem(IDENTITY_STORAGE_KEY);
    }
  } catch {
    // Identity remains session-only when browser storage is unavailable.
  }
  renderIdentityControls();
}

/**
 * Shows whether an identity is stored on this device.
 *
 * When one is, the checkbox has nothing left to ask — it would offer a choice
 * already made — and the pair of buttons collapses to the one that undoes it.
 * The panel then says what it is rather than what it could do.
 */
function renderIdentityControls(): void {
  const stored = readStoredIdentity() !== null;
  elements.rememberOption.hidden = stored;
  elements.rememberIdentity.checked = stored;
  elements.saveIdentity.hidden = stored;
  elements.forgetIdentity.hidden = !stored;
}

function readStoredIdentity(): string | null {
  try {
    return localStorage.getItem(IDENTITY_STORAGE_KEY);
  } catch {
    return null;
  }
}

function saveIdentity(): void {
  const canonical = canonicalIdentityEmail(elements.identityEmail.value);
  if (!canonical) {
    setStateMessage(elements.identityStatus, "invalidIdentity");
    elements.identityEmail.setAttribute("aria-invalid", "true");
    return;
  }
  elements.identityEmail.removeAttribute("aria-invalid");
  state.identityEmail = canonical;
  state.revision += 1;
  invalidateResults();
  elements.identityEmail.value = canonical;
  if (!elements.rememberIdentity.checked) {
    try {
      localStorage.removeItem(IDENTITY_STORAGE_KEY);
    } catch {
      // The value still remains usable for the current tab.
    }
    setStateMessage(elements.identityStatus, "identitySession");
    return;
  }
  try {
    localStorage.setItem(IDENTITY_STORAGE_KEY, canonical);
    setStateMessage(elements.identityStatus, "identitySaved");
    renderIdentityControls();
  } catch {
    setStateMessage(elements.identityStatus, "storageUnavailable");
  }
}

function forgetIdentity(): void {
  state.identityEmail = null;
  state.revision += 1;
  invalidateResults();
  elements.identityEmail.value = "";
  elements.identityEmail.removeAttribute("aria-invalid");
  try {
    localStorage.removeItem(IDENTITY_STORAGE_KEY);
  } catch {
    // The in-memory value has still been cleared.
  }
  setStateMessage(elements.identityStatus, "identityForgotten");
  renderIdentityControls();
  elements.identityEmail.focus();
}

function applyTranslations(): void {
  document.documentElement.lang = state.locale;
  elements.language.value = state.locale;
  elements.theme.value = state.theme;
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n]")) {
    const key = element.dataset.i18n as MessageKey | undefined;
    if (key) element.textContent = t(key);
  }
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n-title]")) {
    const key = element.dataset.i18nTitle as MessageKey | undefined;
    if (key) element.title = t(key);
  }
  for (const element of document.querySelectorAll<HTMLInputElement>("[data-i18n-placeholder]")) {
    const key = element.dataset.i18nPlaceholder as MessageKey | undefined;
    if (key) element.placeholder = t(key);
  }
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n-aria]")) {
    const key = element.dataset.i18nAria as MessageKey | undefined;
    if (key) element.setAttribute("aria-label", t(key));
  }
  for (const element of document.querySelectorAll<HTMLElement>("[data-i18n-state]")) {
    const key = element.dataset.i18nState as MessageKey | undefined;
    if (key) element.textContent = t(key);
  }
  elements.language.setAttribute("aria-label", t("language"));
  elements.addFile.setAttribute("aria-label", t("addFile"));
  elements.addFolder.setAttribute("aria-label", t("addFolder"));
  const route = state.locale === "en" ? "/" : `/${state.locale}/`;
  if (window.location.pathname !== route) {
    window.history.replaceState(
      null,
      "",
      `${route}${window.location.search}${window.location.hash}`,
    );
  }
  const canonical = `https://normfix.vercel.app${route}`;
  document.title = t("seoTitle");
  elements.metaDescription.content = t("seoDescription");
  elements.ogTitle.content = t("seoTitle");
  elements.ogDescription.content = t("seoDescription");
  elements.twitterTitle.content = t("seoTitle");
  elements.twitterDescription.content = t("seoDescription");
  elements.ogUrl.content = canonical;
  const ogLocale =
    state.locale === "pt"
      ? "pt_BR"
      : state.locale === "es"
        ? "es_ES"
        : state.locale === "fr"
          ? "fr_FR"
          : "en_US";
  elements.ogLocale.content = ogLocale;
  ["en_US", "pt_BR", "es_ES", "fr_FR"]
    .filter((locale) => locale !== ogLocale)
    .forEach((locale, index) => {
      const meta = elements.ogAlternates[index];
      if (meta) meta.content = locale;
    });
  elements.canonical.href = canonical;
  elements.manifest.href = `${route}site.webmanifest`;
  elements.brand.href = route;
  elements.docsLink.href = state.locale === "en" ? "/docs/" : `/docs/${state.locale}/`;
  updateEditorMeta();
  renderOfflineStatus();
  if (state.results.size > 0) renderRunResult();
  else resetCopyLabel();
}

function changeLocale(locale: Locale): void {
  state.locale = locale;
  const route = locale === "en" ? "/" : `/${locale}/`;
  window.history.replaceState(null, "", route);
  try {
    localStorage.setItem(LOCALE_STORAGE_KEY, locale);
  } catch {
    // The selected language still applies for the current page.
  }
  applyTranslations();
}

async function loadGitHubStars(): Promise<void> {
  elements.starCount.textContent = String(FALLBACK_STARS);
  try {
    const response = await fetch(GITHUB_REPOSITORY_API, {
      ...githubRequestInit(),
      signal: AbortSignal.timeout(4000),
    });
    if (!response.ok) throw new Error(`GitHub returned ${response.status}`);
    const stars = starCount(await response.json());
    if (stars === null) {
      throw new Error("GitHub returned an invalid star count");
    }
    elements.starCount.textContent = stars.toLocaleString(state.locale);
    elements.starCount.removeAttribute("title");
    delete elements.starCount.dataset.i18nTitle;
  } catch {
    elements.starCount.dataset.i18nTitle = "githubFallback";
    elements.starCount.title = t("githubFallback");
  }
}

function setRuntime(stateName: RuntimeState, label: string): void {
  elements.runtime.dataset.state = stateName;
  delete elements.runtimeLabel.dataset.i18nState;
  elements.runtimeLabel.textContent = label;
}

function setRuntimeMessage(stateName: RuntimeState, key: MessageKey): void {
  setRuntime(stateName, t(key));
  elements.runtimeLabel.dataset.i18nState = key;
}

/**
 * The badge says nothing while offline support is simply working, because that
 * is the normal state of an installed playground. It speaks for the two events
 * that change what the reader can do.
 */
function renderOfflineStatus(): void {
  elements.offlineStatus.dataset.state = state.offlineState;
  const updateReady = state.offlineState === "update-ready";
  elements.offlineUpdate.hidden = !updateReady;
  elements.offlineLabel.textContent = updateReady ? t("offlineUpdate") : t("offlineActive");
  elements.offlineStatus.title = updateReady ? "" : t("offlineActiveTitle");
}

function setStateMessage(element: HTMLElement, key: MessageKey): void {
  element.dataset.i18nState = key;
  element.textContent = t(key);
}

async function loadFormatter(): Promise<void> {
  try {
    const module = (await import("../pkg/normfix_wasm.js")) as WasmModule;
    await module.default();
    state.formatter = module.formatProject;
    elements.run.disabled = false;
    setRuntimeMessage("ready", "wasmReady");
  } catch (error) {
    console.error(error);
    setRuntimeMessage("error", "wasmRequired");
    elements.run.disabled = true;
    elements.run.dataset.i18nTitle = "wasmBuildHelp";
    elements.run.title = t("wasmBuildHelp");
  }
}

function invalidateResults(): void {
  state.results.clear();
  state.selectedResult = null;
  elements.results.hidden = true;
  // The panel is hidden, but an enabled control with nothing to act on is the
  // kind of state that only stays harmless by accident.
  elements.applyAll.disabled = true;
  elements.applyResult.disabled = true;
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
  elements.editorTitle.textContent = path;
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
  for (const [from] of moves) proposed.delete(from);
  for (const [from, to] of moves) {
    const normalized = normalizeSourcePath(to);
    const clashes = [...proposed.keys()].some(
      (loaded) => portablePathKey(loaded) === portablePathKey(normalized),
    );
    if (clashes) throw new Error(t("importConflict", { path: normalized }));
    proposed.set(normalized, state.files.get(from) ?? "");
  }
  validateProjectSources(proposed);
  const selected = moves.find(([from]) => from === state.selected);
  state.files = proposed;
  state.revision += 1;
  invalidateResults();
  const reselect = selected ? normalizeSourcePath(selected[1]) : state.selected;
  if (reselect === null) renderFileList();
  else selectFile(reselect, false);
}

/** Moves one entry, or a whole folder, under `folder`. */
function moveEntry(path: string, isFolder: boolean, folder: string): void {
  if (isFolder && wouldContainItself(path, folder)) return;
  const moves = isFolder
    ? rewritePrefix(state.files.keys(), path, movedPath(path, folder))
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
    applyMoves(isFolder ? rewritePrefix(state.files.keys(), path, target) : [[path, target]]);
  } catch (failure) {
    reportProjectError(failure);
  }
}

/** Removes one entry, or a folder and everything under it. */
function deleteEntry(path: string, isFolder: boolean): void {
  if (state.importing) return;
  syncEditor();
  const removed = isFolder
    ? [...state.files.keys()].filter((loaded) => loaded === path || loaded.startsWith(`${path}/`))
    : [path];
  if (removed.length === 0) return;
  const proposed = new Map(state.files);
  for (const loaded of removed) proposed.delete(loaded);
  state.files = proposed;
  state.revision += 1;
  invalidateResults();
  if (state.selected !== null && removed.includes(state.selected)) {
    const next = [...state.files.keys()].sort()[0];
    if (next === undefined) showEmptyProject();
    else selectFile(next, false);
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
  elements.editorTitle.textContent = "";
  elements.editorMeta.textContent = "";
  elements.editorDisabledTitle.textContent = t("noFilesTitle");
  elements.editorDisabledText.textContent = t("emptyProjectHint");
  elements.editorDisabled.hidden = false;
  elements.run.disabled = true;
  renderFileList();
}

function reportProjectError(failure: unknown): void {
  elements.identityStatus.textContent =
    failure instanceof Error ? failure.message : String(failure);
}

/** The entry the pointer is over, whichever part of the row it landed on. */
function entryOf(target: EventTarget | null): HTMLElement | null {
  return target instanceof Element ? target.closest<HTMLElement>("[data-path]") : null;
}

function renderFileList(): void {
  elements.fileList.replaceChildren();
  closeContextMenu();
  appendTreeLevel(elements.fileList, buildTree([...state.files.keys(), ...state.unsupported]), 0);
  // The panel itself is the project root, so a file dragged clear of every
  // folder comes back out to the top rather than having nowhere to land.
  elements.fileList.dataset.path = "";
  elements.fileList.dataset.kind = "folder";
}

function appendTreeLevel(container: Element, nodes: TreeNode[], depth: number): void {
  for (const node of nodes) {
    const row = document.createElement(node.kind === "file" ? "button" : "div");
    row.className = `file-item file-${node.kind}`;
    row.dataset.path = node.path;
    row.dataset.kind = node.kind;
    row.style.paddingLeft = `${0.5 + depth * 0.85}rem`;
    row.title = node.path;
    row.draggable = true;
    if (row instanceof HTMLButtonElement) {
      row.type = "button";
      row.setAttribute("role", "option");
      row.setAttribute("aria-selected", String(node.path === state.selected));
      if (state.results.get(node.path)?.changed) row.classList.add("changed");
      row.addEventListener("click", () => {
        if (unsupported) showUnsupported(node.path);
        else selectFile(node.path);
      });
    }

    const unsupported = node.kind === "file" && state.unsupported.has(node.path);
    if (unsupported) {
      row.classList.add("file-unsupported");
      row.title = t("unsupportedFile");
    }
    const collapsed = node.kind === "folder" && state.collapsed.has(node.path);
    if (node.kind === "folder") {
      // The whole row opens and closes the folder. A folder has nothing else
      // to be clicked for — it holds no text to edit — so asking the reader to
      // hit a small arrow was asking them to aim at nothing.
      row.setAttribute("role", "button");
      row.tabIndex = 0;
      row.setAttribute("aria-expanded", String(!collapsed));
      row.addEventListener("click", () => toggleFolder(node.path));
      row.addEventListener("keydown", (event) => {
        const pressed = (event as KeyboardEvent).key;
        if (pressed !== "Enter" && pressed !== " ") return;
        event.preventDefault();
        toggleFolder(node.path);
      });
    }

    const mark = document.createElement("span");
    mark.className = node.kind === "file" ? "file-dot" : "folder-mark";
    mark.setAttribute("aria-hidden", "true");
    // The icon is the state: an open folder is open, a closed one is closed.
    if (node.kind === "folder") mark.textContent = collapsed ? FOLDER_SHUT : FOLDER_OPEN;
    const name = document.createElement("span");
    name.className = "file-name";
    name.textContent = node.name;
    row.append(mark, name);
    if (node.kind === "file") {
      const kind = document.createElement("span");
      kind.className = "file-kind";
      kind.textContent = unsupported ? t("unsupportedKind") : fileKind(node.path);
      row.append(kind);
    }
    container.append(row);
    // A closed folder still accepts a drop: the row is there, and dropping on
    // it is how something gets put away without opening it first.
    if (node.kind === "folder" && !collapsed) {
      appendTreeLevel(container, node.children, depth + 1);
    }
  }
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
  elements.editorTitle.textContent = path;
  elements.editorMeta.textContent = "";
  elements.editorDisabledTitle.textContent = t("unsupportedFile");
  elements.editorDisabledText.textContent = t("supportedKinds");
  elements.editorDisabled.hidden = false;
  elements.run.disabled = true;
  renderFileList();
}

/** Turns the editor back on for a file normfix does format. */
function enableEditor(): void {
  elements.editorDisabled.hidden = true;
  elements.run.disabled = state.formatter === null || state.running;
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
  elements.confirmDeleteText.textContent = isFolder
    ? t("deleteFolderText", { path, count: String(count) })
    : t("deleteFileText", { path });
  // The confirm button is wired directly rather than through the dialog's
  // `close` event and its return value: that pair is one indirection more than
  // this needs, and a delete that quietly does nothing is worse than no
  // confirmation at all.
  const confirm = elements.confirmDeleteAction;
  const accept = (): void => {
    elements.confirmDelete.close();
    deleteEntry(path, isFolder);
  };
  confirm.addEventListener("click", accept, { once: true });
  elements.confirmDeleteCancel.addEventListener(
    "click",
    () => confirm.removeEventListener("click", accept),
    { once: true },
  );
  elements.confirmDelete.showModal();
}

/** Opens or closes one folder, keeping whatever is selected selected. */
function toggleFolder(path: string): void {
  if (state.collapsed.has(path)) state.collapsed.delete(path);
  else state.collapsed.add(path);
  renderFileList();
}

function updateEditorMeta(): void {
  const source = state.editor?.getValue() ?? state.files.get(state.selected ?? "") ?? "";
  const lines = source.length === 0 ? 0 : source.split("\n").length;
  const bytes = UTF8_ENCODER.encode(source).length;
  elements.editorMeta.textContent = t("linesBytes", {
    lines,
    bytes: bytes.toLocaleString(state.locale),
  });
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
  if (state.importing || !state.selected || state.files.size === 1) return;
  syncEditor();
  const removed = state.selected;
  state.files.delete(removed);
  state.editor?.removeFile(removed);
  state.revision += 1;
  const next = [...state.files.keys()].sort()[0];
  if (!next) return;
  invalidateResults();
  state.selected = null;
  selectFile(next, false);
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
): Promise<void> {
  if (incoming.length === 0 && unsupported.length === 0) return;
  // A file normfix cannot format is still part of the project the reader
  // dropped in. It is kept by name so the tree can show it and say why it is
  // not open, rather than leaving them to wonder where it went.
  for (const path of unsupported) state.unsupported.add(path);
  if (state.importing) throw new Error(t("importRunning"));
  syncEditor();
  const startingRevision = state.revision;
  state.importing = true;
  setImportControls(true);
  try {
    const candidates = new Map<string, readonly [string, File]>();
    let ignored = unsupported.length;
    let firstRejected: string | null = null;
    for (const { path: rawPath, file } of incoming) {
      if (sourcePathProblem(rawPath) !== null) {
        ignored += 1;
        firstRejected ??= rawPath;
        state.unsupported.add(rawPath);
        continue;
      }
      const path = normalizeSourcePath(rawPath);
      const portableKey = portablePathKey(path);
      if (candidates.has(portableKey)) {
        throw new Error(t("importDuplicate", { path }));
      }
      if ([...state.files.keys()].some((loaded) => portablePathKey(loaded) === portableKey)) {
        throw new Error(t("importConflict", { path }));
      }
      if (file.size > MAX_FILE_BYTES) {
        throw new Error(t("fileTooLarge", { path, count: MAX_FILE_BYTES }));
      }
      candidates.set(portableKey, [path, file]);
    }
    if (candidates.size === 0) {
      // Nothing usable arrived. One rejected path explains itself far better
      // than a count, which matters most when a student dropped a single file.
      if (firstRejected !== null) normalizeSourcePath(firstRejected);
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
    state.revision += 1;
    if (candidates.size > 0) invalidateResults();
    if (imported.selectedPath) selectFile(imported.selectedPath, false);
    renderFileList();
    const added = tPlural("imported", candidates.size);
    setRuntime("ready", ignored > 0 ? `${added} ${tPlural("skipped", ignored)}` : added);
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
}

async function runFormatter(): Promise<void> {
  if (!state.formatter || state.running) return;
  syncEditor();
  state.running = true;
  elements.run.disabled = true;
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
    const response = JSON.parse(state.formatter(JSON.stringify(request))) as PlaygroundResponse;
    if (response.schema_version !== 1 || !Array.isArray(response.files)) {
      throw new Error(t("responseSchema"));
    }
    const inputSources = new Map(request.files.map((file) => [file.path, file.source]));
    state.results = new Map(
      response.files.map((file): [string, ResultRecord] => {
        const inputSource = inputSources.get(file.path);
        if (inputSource === undefined) {
          throw new Error(t("responsePath", { path: file.path }));
        }
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
    elements.results.scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (error) {
    console.error(error);
    setRuntime("error", error instanceof Error ? error.message : String(error));
  } finally {
    state.running = false;
    elements.run.disabled = !state.formatter;
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
  elements.resultFile.replaceChildren();
  for (const file of files) {
    const option = document.createElement("option");
    option.value = file.path;
    option.textContent = file.path;
    elements.resultFile.append(option);
  }
  if (!state.selectedResult || !state.results.has(state.selectedResult)) {
    state.selectedResult = files[0]?.path ?? null;
  }
  elements.resultFile.value = state.selectedResult ?? "";
  renderSelectedResult();
  renderFileList();
  elements.results.hidden = false;
}

function renderSummary(summary: BrowserSummary): void {
  const values: Array<readonly [number, MessageKey]> = [
    [summary.files, "filesSummary"],
    [summary.changed, "changedSummary"],
    [summary.fixes, "fixesSummary"],
    [summary.diagnostics, "diagnosticsSummary"],
    [summary.failed, "failedSummary"],
  ];
  elements.summary.replaceChildren();
  for (const [value, label] of values) {
    const pill = document.createElement("span");
    pill.className = "summary-pill";
    const count = document.createElement("strong");
    count.textContent = String(value);
    pill.append(count, document.createTextNode(t(label)));
    elements.summary.append(pill);
  }
}

function selectedResult(): ResultRecord | undefined {
  if (!state.selectedResult) return undefined;
  const result = state.results.get(state.selectedResult);
  if (!result || state.files.get(result.path) !== result.inputSource) return undefined;
  return result;
}

function renderSelectedResult(): void {
  const result = selectedResult();
  if (!result) return;
  elements.formattedOutput.textContent = result.formatted;
  elements.diffOutput.textContent = result.diff || t("noByteChanges");
  elements.diagnosticCount.textContent = String(result.diagnostics.length);
  elements.applyResult.disabled = Boolean(result.error) || !result.stable;
  elements.applyAll.disabled = applicableResults().length === 0;
  elements.downloadCurrent.disabled = Boolean(result.error) || !result.stable;
  elements.copyCurrent.disabled = Boolean(result.error) || !result.stable;
  resetCopyLabel();
  renderDiagnostics(result);
}

function renderDiagnostics(result: ResultRecord): void {
  elements.diagnosticsView.replaceChildren();
  if (result.error || !result.stable) {
    // Why nothing was formatted goes first, in the reader's language. The
    // findings then follow, because a file that will not parse has the one
    // finding that matters most — where the parser lost its way — and hiding
    // it behind the reason left a reader with a sentence about `ERROR` and
    // `MISSING` bytes and no line to look at.
    elements.diagnosticsView.append(
      emptyState(
        t("fileUnchanged"),
        // A file the parser could not read carries the reason as an error; an
        // unstable run carries none. The error is what tells the two apart —
        // `stable` is false for both, and reading it instead sent every
        // unreadable file the fixed-point message.
        result.error ? t("unparsableFile") : t("unstableFormatter"),
      ),
    );
    if (result.diagnostics.length === 0) return;
  }
  if (result.diagnostics.length === 0) {
    elements.diagnosticsView.append(emptyState(t("noDiagnostics"), t("cliCoverage")));
  } else {
    for (const diagnostic of result.diagnostics) {
      const template = elements.diagnosticTemplate.content.firstElementChild;
      if (!(template instanceof HTMLElement)) {
        throw new Error("Diagnostic template is missing its root element.");
      }
      const card = template.cloneNode(true) as HTMLElement;
      card.dataset.severity = diagnostic.severity;
      requiredChild<HTMLElement>(card, ".severity").textContent = diagnostic.severity;
      requiredChild<HTMLElement>(card, ".rule").textContent = diagnostic.rule_id;
      requiredChild<HTMLElement>(card, ".location").textContent = diagnostic.location
        ? `L${diagnostic.location.line}:C${diagnostic.location.column}`
        : "";
      requiredChild<HTMLElement>(card, ".diagnostic-message").textContent = diagnostic.message;
      const help = requiredChild<HTMLElement>(card, ".diagnostic-help");
      help.textContent = diagnostic.help ? `${t("next")}: ${diagnostic.help}` : diagnostic.source;
      elements.diagnosticsView.append(card);
    }
  }
  if (result.fixes.length > 0) {
    const section = document.createElement("section");
    section.className = "fixes-section";
    const heading = document.createElement("h3");
    heading.textContent = t("fixesApplied", { count: result.fixes.length });
    const list = document.createElement("ul");
    list.className = "fix-list";
    for (const fix of result.fixes) {
      const item = document.createElement("li");
      const rule = document.createElement("code");
      rule.textContent = fix.rule_id;
      item.append(rule, document.createTextNode(fix.description));
      list.append(item);
    }
    section.append(heading, list);
    elements.diagnosticsView.append(section);
  }
  if (result.budget.length > 0) {
    elements.diagnosticsView.append(renderBudget(result.budget));
  }
}

function requiredChild<T extends Element>(parent: ParentNode, selector: string): T {
  const child = parent.querySelector<T>(selector);
  if (!child) throw new Error(`Required child is missing: ${selector}`);
  return child;
}

function emptyState(title: string, message: string): HTMLElement {
  const container = document.createElement("div");
  container.className = "empty-state";
  const content = document.createElement("div");
  const strong = document.createElement("strong");
  strong.textContent = title;
  const text = document.createElement("span");
  text.textContent = message;
  content.append(strong, text);
  container.append(content);
  return container;
}

function renderBudget(budgets: BrowserBudget[]): HTMLElement {
  const section = document.createElement("section");
  section.className = "budget-section";
  const heading = document.createElement("h3");
  heading.textContent = t("functionBudget");
  const table = document.createElement("table");
  table.className = "budget-table";
  const header = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const label of [t("function"), t("line"), t("bodyLines"), t("variables"), t("parameters")]) {
    const cell = document.createElement("th");
    cell.scope = "col";
    cell.textContent = label;
    headerRow.append(cell);
  }
  header.append(headerRow);
  const body = document.createElement("tbody");
  for (const budget of budgets) {
    const row = document.createElement("tr");
    const values = [
      `${budget.function}()`,
      String(budget.line),
      `${budget.lines}/${budget.line_limit}`,
      `${budget.variables}/${budget.variable_limit}`,
      `${budget.parameters}/${budget.parameter_limit}`,
    ];
    const over = [
      false,
      false,
      budget.lines > budget.line_limit,
      budget.variables > budget.variable_limit,
      budget.parameters > budget.parameter_limit,
    ];
    values.forEach((value, index) => {
      const cell = document.createElement("td");
      cell.textContent = value;
      if (over[index]) cell.className = "budget-over";
      row.append(cell);
    });
    body.append(row);
  }
  table.append(header, body);
  section.append(heading, table);
  return section;
}

function activateTab(view: string): void {
  for (const tab of document.querySelectorAll<HTMLButtonElement>("[role=tab][data-view]")) {
    const selected = tab.dataset.view === view;
    tab.setAttribute("aria-selected", String(selected));
    const panel = requiredElement<HTMLElement>(`#${tab.dataset.view}-view`);
    panel.hidden = !selected;
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

function downloadBlob(blob: Blob, filename: string): void {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

let copyLabelTimer: number | undefined;

function resetCopyLabel(): void {
  if (copyLabelTimer !== undefined) {
    window.clearTimeout(copyLabelTimer);
    copyLabelTimer = undefined;
  }
  elements.copyCurrent.textContent = t("copyFile");
}

function flashCopyLabel(label: string): void {
  elements.copyCurrent.textContent = label;
  if (copyLabelTimer !== undefined) window.clearTimeout(copyLabelTimer);
  copyLabelTimer = window.setTimeout(resetCopyLabel, 1600);
}

/// Selects the formatted output so a keyboard copy works.
function selectFormattedOutput(): void {
  const selection = window.getSelection();
  if (!selection) return;
  const range = document.createRange();
  range.selectNodeContents(elements.formattedOutput);
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
  const name = result.path.split("/").at(-1) || "normfix-output.c";
  downloadBlob(new Blob([result.formatted], { type: "text/plain;charset=utf-8" }), name);
}

function downloadAll(): void {
  const files = [...state.results.values()]
    .filter((file) => !file.error && file.stable && state.files.get(file.path) === file.inputSource)
    .map((file) => ({ path: file.path, source: file.formatted }));
  if (files.length === 0) return;
  try {
    const archive = buildZip(files satisfies ProjectSourceFile[]);
    downloadBlob(new Blob([archive], { type: "application/zip" }), "normfix-formatted.zip");
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
  elements.dropOverlay.hidden = !active;
}

window.addEventListener("dragenter", (event) => {
  if (!dragCarriesFiles(event)) return;
  event.preventDefault();
  dragDepth += 1;
  elements.dropOverlay.hidden = false;
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
  await loadFiles(selection.files, selection.unsupported);
}

elements.filePicker.addEventListener("change", () => {
  const chosen = [...(elements.filePicker.files ?? [])].map((file): DroppedFile => ({
    path: file.webkitRelativePath || file.name,
    file,
  }));
  void loadFiles(chosen).catch((error: unknown) => {
    setRuntime("error", error instanceof Error ? error.message : String(error));
  });
});
elements.removeFile.addEventListener("click", removeSelected);
/// The name a kind starts from, keeping any folder the reader already typed.
///
/// `.c` is not the only thing this page formats, and typing the extension by
/// hand is how a reader discovers that only by getting it wrong. Picking a kind
/// rewrites the name, and keeping the folder means choosing a kind after typing
/// `src/utils.c` does not throw the folder away.

// Dragging an entry onto a folder, and the menu that renames or deletes one.
//
// The panel itself carries the root prefix, so an entry dragged clear of every
// folder lands at the top of the project rather than nowhere. A drop is read
// from the row under the pointer, never from the row the drag started on.
let contextMenu: HTMLElement | null = null;

function closeContextMenu(): void {
  contextMenu?.remove();
  contextMenu = null;
}

function dropTargetOf(target: EventTarget | null): { path: string; kind: string } | null {
  const entry = entryOf(target);
  if (!entry) return null;
  // Dropping on a file means dropping into the folder that holds it, which is
  // what the pointer looks like it is doing.
  const path = entry.dataset.path ?? "";
  if (entry.dataset.kind === "folder") return { path, kind: "folder" };
  const parent = path.includes("/") ? path.slice(0, path.lastIndexOf("/")) : "";
  return { path: parent, kind: "folder" };
}

elements.fileList.addEventListener("dragstart", (event) => {
  const entry = entryOf(event.target);
  if (!entry?.dataset.path) return;
  event.dataTransfer?.setData("text/normfix-entry", `${entry.dataset.kind}:${entry.dataset.path}`);
  if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
});

elements.fileList.addEventListener("dragover", (event) => {
  if (!event.dataTransfer?.types.includes("text/normfix-entry")) return;
  event.preventDefault();
  event.dataTransfer.dropEffect = "move";
  const entry = entryOf(event.target);
  for (const row of elements.fileList.querySelectorAll(".drop-into")) {
    row.classList.remove("drop-into");
  }
  if (entry?.dataset.kind === "folder") entry.classList.add("drop-into");
});

elements.fileList.addEventListener("dragleave", (event) => {
  entryOf(event.target)?.classList.remove("drop-into");
});

elements.fileList.addEventListener("drop", (event) => {
  const payload = event.dataTransfer?.getData("text/normfix-entry");
  if (!payload) return;
  event.preventDefault();
  for (const row of elements.fileList.querySelectorAll(".drop-into")) {
    row.classList.remove("drop-into");
  }
  const separator = payload.indexOf(":");
  const kind = payload.slice(0, separator);
  const path = payload.slice(separator + 1);
  const destination = dropTargetOf(event.target);
  if (destination) moveEntry(path, kind === "folder", destination.path);
});

elements.fileList.addEventListener("contextmenu", (event) => {
  const entry = entryOf(event.target);
  if (!entry?.dataset.path) return;
  event.preventDefault();
  openContextMenu(entry, event.clientX, event.clientY);
});

function openContextMenu(entry: HTMLElement, x: number, y: number): void {
  closeContextMenu();
  const path = entry.dataset.path ?? "";
  const isFolder = entry.dataset.kind === "folder";
  const menu = document.createElement("div");
  menu.className = "context-menu";
  menu.setAttribute("role", "menu");
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;

  const action = (label: string, run: () => void): void => {
    const button = document.createElement("button");
    button.type = "button";
    button.setAttribute("role", "menuitem");
    button.textContent = label;
    button.addEventListener("click", () => {
      closeContextMenu();
      run();
    });
    menu.append(button);
  };

  action(t("renameEntry"), () => startRename(entry, path, isFolder));
  action(t("deleteEntry"), () => confirmDelete(path, isFolder));
  document.body.append(menu);
  contextMenu = menu;
  menu.querySelector("button")?.focus();
}

/** Turns a row into its own name field, the way the draft row already reads. */
function startRename(entry: HTMLElement, path: string, isFolder: boolean): void {
  const label = entry.querySelector<HTMLElement>(".file-name");
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

document.addEventListener("click", (event) => {
  if (contextMenu && !contextMenu.contains(event.target as Node)) closeContextMenu();
});
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") closeContextMenu();
});

elements.addFile.addEventListener("click", () => openDraft("file"));
elements.addFolder.addEventListener("click", () => openDraft("folder"));
elements.resultFile.addEventListener("change", () => {
  state.selectedResult = elements.resultFile.value;
  renderSelectedResult();
});
elements.applyResult.addEventListener("click", applySelectedResult);
elements.applyAll.addEventListener("click", applyAllResults);
elements.copyCurrent.addEventListener("click", () => {
  void copyCurrent();
});
elements.downloadCurrent.addEventListener("click", downloadCurrent);
elements.downloadAll.addEventListener("click", downloadAll);
for (const tab of document.querySelectorAll<HTMLButtonElement>("[role=tab][data-view]")) {
  tab.addEventListener("click", () => {
    if (tab.dataset.view) activateTab(tab.dataset.view);
  });
}
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
  elements.theme.value = preference;
}

elements.theme.addEventListener("change", () => {
  const preference = elements.theme.value;
  if (isThemePreference(preference)) changeTheme(preference);
});

// Only meaningful while following the system, but harmless otherwise: the
// preference is what decides, and re-applying it is idempotent.
watchSystemAppearance(() => {
  if (state.theme === "system") changeTheme("system");
});

elements.language.addEventListener("change", () => {
  const locale = elements.language.value as Locale;
  if (SUPPORTED_LOCALES.includes(locale)) changeLocale(locale);
});
elements.offlineUpdate.addEventListener("click", () => {
  state.offlineSupport?.applyUpdate();
});
elements.saveIdentity.addEventListener("click", saveIdentity);
elements.forgetIdentity.addEventListener("click", forgetIdentity);
elements.identityEmail.addEventListener("keydown", (event) => {
  if (event.key === "Enter") {
    event.preventDefault();
    saveIdentity();
  }
});

async function initialize(): Promise<void> {
  state.appearance = applyThemePreference(state.theme);
  applyTranslations();
  loadIdentity();
  state.offlineSupport = startOfflineSupport({
    onState: (offlineState) => {
      state.offlineState = offlineState;
      renderOfflineStatus();
    },
    onConnectivity: (online) => {
      elements.offlineStatus.dataset.online = String(online);
    },
  });
  renderFileList();
  const formatterPromise = loadFormatter();
  state.editor = await createSourceEditor(
    elements.editorContainer,
    elements.fallbackEditor,
    "main.c",
    SAMPLE,
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
  updateEditorMeta();
  await formatterPromise;
  whenIdle(() => void loadGitHubStars());
}

/**
 * Runs work that nothing on the page is waiting for.
 *
 * The star count is decoration: it is worth a request, but not worth competing
 * with the formatter for a phone's one slow connection. Waiting for load and
 * then for an idle moment takes it off the critical path entirely, and the
 * bundled count is already on screen until it answers.
 */
function whenIdle(work: () => void): void {
  const idle = window.requestIdleCallback as typeof window.requestIdleCallback | undefined;
  const schedule = () => {
    if (idle) idle(() => work(), { timeout: 3000 });
    else window.setTimeout(work, 1000);
  };
  if (document.readyState === "complete") schedule();
  else window.addEventListener("load", schedule, { once: true });
}

void initialize().catch((error: unknown) => {
  console.error(error);
  setRuntime("error", error instanceof Error ? error.message : String(error));
});
