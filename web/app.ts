import { createSourceEditor, type SourceEditor } from "./editor";
import {
  SUPPORTED_LOCALES,
  detectLocale,
  translate,
  type Locale,
  type MessageKey,
} from "./i18n";
import {
  ImportBatchError,
  MAX_FILES,
  MAX_FILE_BYTES,
  MAX_PROJECT_BYTES,
  TarArchiveError,
  buildTar,
  canonicalIdentityEmail,
  portablePathKey,
  readImportBatch,
  sourcePathProblem,
  type ProjectSourceFile,
} from "./project-files";
import { GITHUB_REPOSITORY_API, githubRequestInit, starCount } from "./github";

const UTF8_ENCODER = new TextEncoder();
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
};

const elements = {
  runtime: requiredElement<HTMLElement>("#runtime-status"),
  runtimeLabel: requiredElement<HTMLElement>("#runtime-label"),
  fileList: requiredElement<HTMLElement>("#file-list"),
  filePicker: requiredElement<HTMLInputElement>("#file-picker"),
  addFile: requiredElement<HTMLButtonElement>("#add-file"),
  removeFile: requiredElement<HTMLButtonElement>("#remove-file"),
  editorContainer: requiredElement<HTMLElement>("#monaco-editor"),
  fallbackEditor: requiredElement<HTMLTextAreaElement>("#fallback-editor"),
  editorTitle: requiredElement<HTMLElement>("#editor-title"),
  editorMeta: requiredElement<HTMLElement>("#editor-meta"),
  run: requiredElement<HTMLButtonElement>("#run"),
  results: requiredElement<HTMLElement>("#results"),
  summary: requiredElement<HTMLElement>("#summary"),
  resultFile: requiredElement<HTMLSelectElement>("#result-file"),
  applyResult: requiredElement<HTMLButtonElement>("#apply-result"),
  copyCurrent: requiredElement<HTMLButtonElement>("#copy-current"),
  downloadCurrent: requiredElement<HTMLButtonElement>("#download-current"),
  downloadAll: requiredElement<HTMLButtonElement>("#download-all"),
  formattedOutput: requiredElement<HTMLElement>("#formatted-output"),
  diffOutput: requiredElement<HTMLElement>("#diff-output"),
  diagnosticsView: requiredElement<HTMLElement>("#diagnostics-view"),
  diagnosticCount: requiredElement<HTMLElement>("#diagnostic-count"),
  diagnosticTemplate: requiredElement<HTMLTemplateElement>("#diagnostic-template"),
  dialog: requiredElement<HTMLDialogElement>("#new-file-dialog"),
  newFileForm: requiredElement<HTMLFormElement>("#new-file-form"),
  newFileName: requiredElement<HTMLInputElement>("#new-file-name"),
  newFileError: requiredElement<HTMLElement>("#new-file-error"),
  language: requiredElement<HTMLSelectElement>("#language"),
  identityEmail: requiredElement<HTMLInputElement>("#identity-email"),
  rememberIdentity: requiredElement<HTMLInputElement>("#remember-identity"),
  saveIdentity: requiredElement<HTMLButtonElement>("#save-identity"),
  forgetIdentity: requiredElement<HTMLButtonElement>("#forget-identity"),
  identityStatus: requiredElement<HTMLElement>("#identity-status"),
  starCount: requiredElement<HTMLElement>("#star-count"),
  docsLink: requiredElement<HTMLAnchorElement>("#docs-link"),
  brand: requiredElement<HTMLAnchorElement>(".brand"),
  canonical: requiredElement<HTMLLinkElement>("#canonical-url"),
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
  elements.identityEmail.focus();
}

function applyTranslations(): void {
  document.documentElement.lang = state.locale;
  elements.language.value = state.locale;
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
  const ogLocale = state.locale === "pt"
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
  elements.brand.href = route;
  elements.docsLink.href = state.locale === "en" ? "/docs/" : `/docs/${state.locale}/`;
  updateEditorMeta();
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

function setStateMessage(element: HTMLElement, key: MessageKey): void {
  element.dataset.i18nState = key;
  element.textContent = t(key);
}

async function loadFormatter(): Promise<void> {
  try {
    const module = (await import("./pkg/normfix_wasm.js")) as WasmModule;
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
  const source = state.files.get(path);
  if (source === undefined) return;
  state.selected = path;
  state.editor?.setFile(path, source);
  elements.editorTitle.textContent = path;
  updateEditorMeta();
  renderFileList();
}

function renderFileList(): void {
  elements.fileList.replaceChildren();
  for (const path of [...state.files.keys()].sort()) {
    const item = document.createElement("button");
    item.type = "button";
    item.className = "file-item";
    item.setAttribute("role", "option");
    item.setAttribute("aria-selected", String(path === state.selected));
    item.title = path;
    if (state.results.get(path)?.changed) item.classList.add("changed");

    const dot = document.createElement("span");
    dot.className = "file-dot";
    dot.setAttribute("aria-hidden", "true");
    const name = document.createElement("span");
    name.className = "file-name";
    name.textContent = path;
    const kind = document.createElement("span");
    kind.className = "file-kind";
    kind.textContent = fileKind(path);
    item.append(dot, name, kind);
    item.addEventListener("click", () => selectFile(path));
    elements.fileList.append(item);
  }
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
  if (problem.code === "tar_path") throw new Error(t("tarPath"));
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
  if ([...state.files.keys()].some((loaded) => portablePathKey(loaded) === portablePathKey(normalized))) {
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

async function loadFiles(fileList: FileList | null): Promise<void> {
  if (!fileList || fileList.length === 0) return;
  if (state.importing) throw new Error(t("importRunning"));
  syncEditor();
  const startingRevision = state.revision;
  state.importing = true;
  setImportControls(true);
  try {
    const candidates = new Map<string, readonly [string, File]>();
    for (const file of fileList) {
      const path = normalizeSourcePath(file.webkitRelativePath || file.name);
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
  } finally {
    state.importing = false;
    setImportControls(false);
    elements.filePicker.value = "";
  }
}

function setImportControls(disabled: boolean): void {
  elements.filePicker.disabled = disabled;
  elements.addFile.disabled = disabled;
  elements.removeFile.disabled = disabled;
}

async function runFormatter(): Promise<void> {
  if (!state.formatter || state.running) return;
  syncEditor();
  state.running = true;
  elements.run.disabled = true;
  setRuntimeMessage("loading", "formatting");
  try {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
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
    state.selectedResult = state.selected && state.results.has(state.selected)
      ? state.selected
      : response.files[0]?.path ?? null;
    renderRunResult(response.summary);
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
  elements.downloadCurrent.disabled = Boolean(result.error) || !result.stable;
  elements.copyCurrent.disabled = Boolean(result.error) || !result.stable;
  resetCopyLabel();
  renderDiagnostics(result);
}

function renderDiagnostics(result: ResultRecord): void {
  elements.diagnosticsView.replaceChildren();
  if (result.error || !result.stable) {
    elements.diagnosticsView.append(
      emptyState(
        t("fileUnchanged"),
        result.error || t("unstableFormatter"),
      ),
    );
    return;
  }
  if (result.diagnostics.length === 0) {
    elements.diagnosticsView.append(
      emptyState(t("noDiagnostics"), t("cliCoverage")),
    );
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
  for (const label of [
    t("function"),
    t("line"),
    t("bodyLines"),
    t("variables"),
    t("parameters"),
  ]) {
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
  state.files.set(result.path, result.formatted);
  state.revision += 1;
  invalidateResults();
  selectFile(result.path, false);
  state.editor?.focus();
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
    .filter(
      (file) =>
        !file.error && file.stable && state.files.get(file.path) === file.inputSource,
    )
    .map((file) => ({ path: file.path, source: file.formatted }));
  if (files.length === 0) return;
  try {
    const archive = buildTar(files satisfies ProjectSourceFile[]);
    downloadBlob(
      new Blob([archive], { type: "application/x-tar" }),
      "normfix-formatted.tar",
    );
  } catch (error) {
    if (error instanceof TarArchiveError && error.code === "path_too_long" && error.path) {
      setRuntime("error", t("downloadPath", { path: error.path }));
      return;
    }
    if (error instanceof TarArchiveError) {
      setRuntime("error", t("archiveField"));
      return;
    }
    throw error;
  }
}

elements.run.addEventListener("click", runFormatter);
elements.filePicker.addEventListener("change", () => {
  void loadFiles(elements.filePicker.files).catch((error: unknown) => {
    setRuntime("error", error instanceof Error ? error.message : String(error));
  });
});
elements.removeFile.addEventListener("click", removeSelected);
elements.addFile.addEventListener("click", () => {
  elements.newFileError.textContent = "";
  elements.newFileName.value = "new_file.c";
  elements.dialog.showModal();
  elements.newFileName.select();
});
elements.newFileForm.addEventListener("submit", (event) => {
  if (!(event instanceof SubmitEvent)) return;
  if (!(event.submitter instanceof HTMLButtonElement) || event.submitter.value !== "create") {
    return;
  }
  event.preventDefault();
  try {
    addSource(elements.newFileName.value.trim(), "");
    elements.dialog.close();
    state.editor?.focus();
  } catch (error) {
    elements.newFileError.textContent = error instanceof Error ? error.message : String(error);
  }
});
elements.resultFile.addEventListener("change", () => {
  state.selectedResult = elements.resultFile.value;
  renderSelectedResult();
});
elements.applyResult.addEventListener("click", applySelectedResult);
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
elements.language.addEventListener("change", () => {
  const locale = elements.language.value as Locale;
  if (SUPPORTED_LOCALES.includes(locale)) changeLocale(locale);
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
  applyTranslations();
  loadIdentity();
  renderFileList();
  const formatterPromise = loadFormatter();
  state.editor = await createSourceEditor(
    elements.editorContainer,
    elements.fallbackEditor,
    "main.c",
    SAMPLE,
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
  void loadGitHubStars();
}

void initialize().catch((error: unknown) => {
  console.error(error);
  setRuntime("error", error instanceof Error ? error.message : String(error));
});
