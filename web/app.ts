const MAX_FILES = 128;
const MAX_PATH_BYTES = 240;
const MAX_FILE_BYTES = 1024 * 1024;
const MAX_PROJECT_BYTES = 4 * 1024 * 1024;
const UTF8_ENCODER = new TextEncoder();
const UTF8_DECODER = new TextDecoder("utf-8", { fatal: true });

const SAMPLE: string = `#include <unistd.h>

int main(void)
{
    if (write(1, "normfix\\n", 8) > 0) { return (0); }
    else { return (1); }
}
`;

type RuntimeState = "loading" | "ready" | "error";
type Severity = "error" | "warning" | "info";

interface SourceFile {
  path: string;
  source: string;
}

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
  running: boolean;
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
  running: false,
};

const elements = {
  runtime: requiredElement<HTMLElement>("#runtime-status"),
  runtimeLabel: requiredElement<HTMLElement>("#runtime-label"),
  fileList: requiredElement<HTMLElement>("#file-list"),
  filePicker: requiredElement<HTMLInputElement>("#file-picker"),
  addFile: requiredElement<HTMLButtonElement>("#add-file"),
  removeFile: requiredElement<HTMLButtonElement>("#remove-file"),
  editor: requiredElement<HTMLTextAreaElement>("#editor"),
  editorTitle: requiredElement<HTMLElement>("#editor-title"),
  editorMeta: requiredElement<HTMLElement>("#editor-meta"),
  run: requiredElement<HTMLButtonElement>("#run"),
  results: requiredElement<HTMLElement>("#results"),
  summary: requiredElement<HTMLElement>("#summary"),
  resultFile: requiredElement<HTMLSelectElement>("#result-file"),
  applyResult: requiredElement<HTMLButtonElement>("#apply-result"),
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
};

function setRuntime(stateName: RuntimeState, label: string): void {
  elements.runtime.dataset.state = stateName;
  elements.runtimeLabel.textContent = label;
}

async function loadFormatter(): Promise<void> {
  try {
    const module = (await import("./pkg/normfix_wasm.js")) as WasmModule;
    await module.default();
    state.formatter = module.formatProject;
    elements.run.disabled = false;
    setRuntime("ready", "WASM ready");
  } catch (error) {
    console.error(error);
    setRuntime("error", "WASM build required");
    elements.run.disabled = true;
    elements.run.title = "Build the playground module first; see web/README.md.";
  }
}

function invalidateResults(): void {
  state.results.clear();
  state.selectedResult = null;
  elements.results.hidden = true;
}

function syncEditor(): void {
  if (state.selected && state.files.get(state.selected) !== elements.editor.value) {
    state.files.set(state.selected, elements.editor.value);
    invalidateResults();
  }
}

function selectFile(path: string, syncCurrent = true): void {
  if (syncCurrent) syncEditor();
  if (!state.files.has(path)) return;
  state.selected = path;
  elements.editor.value = state.files.get(path) ?? "";
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
    kind.textContent = path.endsWith(".h") ? "H" : "C";
    item.append(dot, name, kind);
    item.addEventListener("click", () => selectFile(path));
    elements.fileList.append(item);
  }
}

function updateEditorMeta(): void {
  const source = elements.editor.value;
  const lines = source.length === 0 ? 0 : source.split("\n").length;
  const bytes = new TextEncoder().encode(source).length;
  elements.editorMeta.textContent = `${lines} line${lines === 1 ? "" : "s"} · ${bytes.toLocaleString()} bytes`;
}

function normalizeSourcePath(path: string): string {
  const normalized = path.replaceAll("\\", "/").replace(/^\.\//, "");
  if (!/\.(c|h)$/.test(normalized)) {
    throw new Error("Only .c and .h files are supported.");
  }
  if (
    normalized.length === 0 ||
    normalized.startsWith("/") ||
    normalized.includes(":") ||
    normalized.split("/").some((part) => part === "" || part === "." || part === "..") ||
    [...normalized].some((character) => /[\u0000-\u001f\u007f]/.test(character))
  ) {
    throw new Error("Use a printable relative path without empty, current, or parent segments.");
  }
  if (UTF8_ENCODER.encode(normalized).length > MAX_PATH_BYTES) {
    throw new Error(`Paths must fit within ${MAX_PATH_BYTES} UTF-8 bytes.`);
  }
  if (!portableTarPath(normalized)) {
    throw new Error(
      "Paths must fit the portable tar name fields (100-byte name and 155-byte prefix).",
    );
  }
  return normalized;
}

function portableTarPath(path: string): boolean {
  if (UTF8_ENCODER.encode(path).length <= 100) return true;
  for (let separator = path.lastIndexOf("/"); separator >= 0; separator = path.lastIndexOf("/", separator - 1)) {
    const prefixBytes = UTF8_ENCODER.encode(path.slice(0, separator)).length;
    const nameBytes = UTF8_ENCODER.encode(path.slice(separator + 1)).length;
    if (prefixBytes <= 155 && nameBytes <= 100) return true;
  }
  return false;
}

function validateProjectSources(files: ReadonlyMap<string, string>): void {
  if (files.size === 0) throw new Error("Select or add at least one .c or .h file.");
  if (files.size > MAX_FILES) {
    throw new Error(`The playground accepts at most ${MAX_FILES} files per run.`);
  }
  let projectBytes = 0;
  for (const [path, source] of files) {
    normalizeSourcePath(path);
    const fileBytes = UTF8_ENCODER.encode(source).length;
    if (fileBytes > MAX_FILE_BYTES) {
      throw new Error(`${path} exceeds the ${MAX_FILE_BYTES}-byte browser limit.`);
    }
    projectBytes += fileBytes;
    if (projectBytes > MAX_PROJECT_BYTES) {
      throw new Error(`The selected sources exceed the ${MAX_PROJECT_BYTES}-byte project limit.`);
    }
  }
}

function addSource(path: string, source = ""): void {
  syncEditor();
  const normalized = normalizeSourcePath(path);
  if (state.files.has(normalized)) {
    throw new Error(`${normalized} is already loaded.`);
  }
  const proposed = new Map(state.files);
  proposed.set(normalized, source);
  validateProjectSources(proposed);
  state.files = proposed;
  invalidateResults();
  selectFile(normalized, false);
}

function removeSelected(): void {
  if (!state.selected || state.files.size === 1) return;
  syncEditor();
  const removed = state.selected;
  state.files.delete(removed);
  const next = [...state.files.keys()].sort()[0];
  if (!next) return;
  invalidateResults();
  state.selected = null;
  selectFile(next, false);
}

async function loadFiles(fileList: FileList | null): Promise<void> {
  if (!fileList) return;
  syncEditor();
  const candidates = new Map<string, File>();
  for (const file of fileList) {
    if (!/\.(c|h)$/.test(file.name)) continue;
    const path = normalizeSourcePath(file.webkitRelativePath || file.name);
    if (file.size > MAX_FILE_BYTES) {
      throw new Error(`${path} exceeds the ${MAX_FILE_BYTES}-byte browser limit.`);
    }
    candidates.set(path, file);
  }
  const projectedPaths = new Set([...state.files.keys(), ...candidates.keys()]);
  if (projectedPaths.size > MAX_FILES) {
    throw new Error(`The playground accepts at most ${MAX_FILES} files per run.`);
  }
  let projectedBytes = [...state.files.values()].reduce(
    (total, source) => total + UTF8_ENCODER.encode(source).length,
    0,
  );
  for (const [path, file] of candidates) {
    projectedBytes -= UTF8_ENCODER.encode(state.files.get(path) ?? "").length;
    projectedBytes += file.size;
  }
  if (projectedBytes > MAX_PROJECT_BYTES) {
    throw new Error(`The selected sources exceed the ${MAX_PROJECT_BYTES}-byte project limit.`);
  }

  const proposed = new Map(state.files);
  let selectedPath: string | null = null;
  for (const [path, file] of candidates) {
    let source: string;
    try {
      source = UTF8_DECODER.decode(await file.arrayBuffer());
    } catch {
      throw new Error(`${path} is not valid UTF-8 source text.`);
    }
    proposed.set(path, source);
    selectedPath = path;
  }
  validateProjectSources(proposed);
  state.files = proposed;
  if (candidates.size > 0) invalidateResults();
  if (selectedPath) selectFile(selectedPath, false);
  renderFileList();
  elements.filePicker.value = "";
}

async function runFormatter(): Promise<void> {
  if (!state.formatter || state.running) return;
  syncEditor();
  state.running = true;
  elements.run.disabled = true;
  setRuntime("loading", "Formatting in this tab…");
  try {
    await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
    validateProjectSources(state.files);
    const request = {
      files: [...state.files.entries()].map(([path, source]) => ({ path, source })),
    };
    const response = JSON.parse(state.formatter(JSON.stringify(request))) as PlaygroundResponse;
    const inputSources = new Map(request.files.map((file) => [file.path, file.source]));
    state.results = new Map(
      response.files.map((file): [string, ResultRecord] => {
        const inputSource = inputSources.get(file.path);
        if (inputSource === undefined) {
          throw new Error(`Formatter returned an unknown path: ${file.path}`);
        }
        return [file.path, { ...file, inputSource }];
      }),
    );
    state.selectedResult = state.selected && state.results.has(state.selected)
      ? state.selected
      : response.files[0]?.path ?? null;
    renderRunResult(response.summary);
    setRuntime("ready", "WASM ready");
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
  const values: Array<readonly [number, string]> = [
    [summary.files, "files"],
    [summary.changed, "changed"],
    [summary.fixes, "fixes"],
    [summary.diagnostics, "diagnostics"],
    [summary.failed, "failed"],
  ];
  elements.summary.replaceChildren();
  for (const [value, label] of values) {
    const pill = document.createElement("span");
    pill.className = "summary-pill";
    const count = document.createElement("strong");
    count.textContent = String(value);
    pill.append(count, document.createTextNode(label));
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
  elements.diffOutput.textContent = result.diff || "No byte changes proposed.";
  elements.diagnosticCount.textContent = String(result.diagnostics.length);
  elements.applyResult.disabled = Boolean(result.error) || !result.stable;
  elements.downloadCurrent.disabled = Boolean(result.error) || !result.stable;
  renderDiagnostics(result);
}

function renderDiagnostics(result: ResultRecord): void {
  elements.diagnosticsView.replaceChildren();
  if (result.error || !result.stable) {
    elements.diagnosticsView.append(
      emptyState(
        "This file was left unchanged",
        result.error || "The formatter did not reach a fixed point; partial output was discarded.",
      ),
    );
    return;
  }
  if (result.diagnostics.length === 0) {
    elements.diagnosticsView.append(
      emptyState("No native diagnostics remain", "Run the desktop CLI for official checker and compiler coverage."),
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
      help.textContent = diagnostic.help ? `Next: ${diagnostic.help}` : diagnostic.source;
      elements.diagnosticsView.append(card);
    }
  }
  if (result.fixes.length > 0) {
    const section = document.createElement("section");
    section.className = "fixes-section";
    const heading = document.createElement("h3");
    heading.textContent = `Applied safely in memory (${result.fixes.length})`;
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
  heading.textContent = "Function budget";
  const table = document.createElement("table");
  table.className = "budget-table";
  const header = document.createElement("thead");
  const headerRow = document.createElement("tr");
  for (const label of ["Function", "Line", "Body lines", "Variables", "Parameters"]) {
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
  invalidateResults();
  selectFile(result.path, false);
  elements.editor.focus();
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
  downloadBlob(new Blob([buildTar(files)], { type: "application/x-tar" }), "normfix-formatted.tar");
}

function buildTar(files: SourceFile[]): Uint8Array<ArrayBuffer> {
  const encoder = new TextEncoder();
  const chunks: Uint8Array<ArrayBuffer>[] = [];
  for (const file of files) {
    const content = encoder.encode(file.source);
    const header = new Uint8Array(512);
    const [name, prefix] = splitTarPath(file.path, encoder);
    writeTarText(header, 0, 100, name, encoder);
    writeTarText(header, 100, 8, "0000644\0", encoder);
    writeTarText(header, 108, 8, "0000000\0", encoder);
    writeTarText(header, 116, 8, "0000000\0", encoder);
    writeTarText(header, 124, 12, `${content.length.toString(8).padStart(11, "0")}\0`, encoder);
    writeTarText(header, 136, 12, "00000000000\0", encoder);
    header.fill(32, 148, 156);
    header[156] = "0".charCodeAt(0);
    writeTarText(header, 257, 6, "ustar\u0000", encoder);
    writeTarText(header, 263, 2, "00", encoder);
    writeTarText(header, 265, 32, "normfix", encoder);
    writeTarText(header, 297, 32, "normfix", encoder);
    writeTarText(header, 345, 155, prefix, encoder);
    const checksum = header.reduce((total, byte) => total + byte, 0);
    writeTarText(header, 148, 8, `${checksum.toString(8).padStart(6, "0")}\0 `, encoder);
    chunks.push(header, content);
    const padding = (512 - (content.length % 512)) % 512;
    if (padding) chunks.push(new Uint8Array(padding));
  }
  chunks.push(new Uint8Array(1024));
  const size = chunks.reduce((total, chunk) => total + chunk.length, 0);
  const archive = new Uint8Array(size);
  let offset = 0;
  for (const chunk of chunks) {
    archive.set(chunk, offset);
    offset += chunk.length;
  }
  return archive;
}

function splitTarPath(path: string, encoder: TextEncoder): [string, string] {
  if (encoder.encode(path).length <= 100) return [path, ""];
  const separators = [...path.matchAll(/\//g)].map((match) => match.index).reverse();
  for (const separator of separators) {
    const prefix = path.slice(0, separator);
    const name = path.slice(separator + 1);
    if (encoder.encode(prefix).length <= 155 && encoder.encode(name).length <= 100) {
      return [name, prefix];
    }
  }
  throw new Error(`Path is too long for a portable tar archive: ${path}`);
}

function writeTarText(
  buffer: Uint8Array<ArrayBuffer>,
  offset: number,
  length: number,
  value: string,
  encoder: TextEncoder,
): void {
  const encoded = encoder.encode(value);
  if (encoded.length > length) throw new Error("Tar field is too long.");
  buffer.set(encoded, offset);
}

elements.editor.addEventListener("input", () => {
  syncEditor();
  updateEditorMeta();
});
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
    elements.editor.focus();
  } catch (error) {
    elements.newFileError.textContent = error instanceof Error ? error.message : String(error);
  }
});
elements.resultFile.addEventListener("change", () => {
  state.selectedResult = elements.resultFile.value;
  renderSelectedResult();
});
elements.applyResult.addEventListener("click", applySelectedResult);
elements.downloadCurrent.addEventListener("click", downloadCurrent);
elements.downloadAll.addEventListener("click", downloadAll);
for (const tab of document.querySelectorAll<HTMLButtonElement>("[role=tab][data-view]")) {
  tab.addEventListener("click", () => {
    if (tab.dataset.view) activateTab(tab.dataset.view);
  });
}
document.addEventListener("keydown", (event) => {
  if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
    event.preventDefault();
    runFormatter();
  }
});

elements.editor.value = SAMPLE;
renderFileList();
updateEditorMeta();
loadFormatter();
