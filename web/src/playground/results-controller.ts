import { mount } from "svelte";
import Diagnostics from "../components/Diagnostics.svelte";
import ResultSummary from "../components/ResultSummary.svelte";
import { requiredElement, type PlaygroundElements } from "../dom";
import { downloadProject, downloadSource } from "../downloads";
import {
  type BrowserSummary,
  FormatterResponseError,
  parseFormatterResponse,
} from "../formatter-response";
import { ZipArchiveError } from "../project/archive";
import { markersFor } from "../project/markers";
import { codeState, diagnosticsState, resultState } from "../tree-state.svelte";
import type { AppState, PluralTranslator, ResultRecord, Translator } from "./model";

type ResultView = "formatted" | "diagnostics" | "diff";

export interface ResultsController {
  invalidateResults(): void;
  runFormatter(): Promise<void>;
  refreshTranslation(): void;
  setFormattedElement(element: HTMLElement): void;
}

export interface ResultsControllerOptions {
  state: AppState;
  elements: PlaygroundElements;
  translate: Translator;
  translatePlural: PluralTranslator;
  setRuntime: (state: "loading" | "ready" | "error", label: string) => void;
  setRuntimeMessage: (
    state: "loading" | "ready" | "error",
    key: "formatting" | "wasmReady",
  ) => void;
  refreshRunControl: () => void;
  scheduleSave: () => void;
  syncEditor: () => void;
  validateSources: (files: ReadonlyMap<string, string>) => void;
  renderFileList: () => void;
  selectFile: (path: string, syncCurrent?: boolean) => void;
}

function formatHeaderTimestamp(date: Date): string {
  const part = (value: number): string => String(value).padStart(2, "0");
  return `${date.getFullYear()}/${part(date.getMonth() + 1)}/${part(date.getDate())} ${part(date.getHours())}:${part(date.getMinutes())}:${part(date.getSeconds())}`;
}

export function createResultsController(options: ResultsControllerOptions): ResultsController {
  const {
    state,
    elements,
    translate: t,
    translatePlural: tPlural,
    setRuntime,
    setRuntimeMessage,
    refreshRunControl,
    scheduleSave,
    syncEditor,
    validateSources,
    renderFileList,
    selectFile,
  } = options;
  let resultMounted = false;
  let diagnosticsMounted = false;
  let formattedElement: HTMLElement | null = null;
  let copyLabelTimer: number | undefined;

  function invalidateResults(): void {
    scheduleSave();
    elements.results.hidden = true;
    if (state.results.size === 0 && state.selectedResult === null) return;
    state.results.clear();
    state.selectedResult = null;
    resultState.usable = false;
    resultState.applicable = 0;
    paintMarkers();
  }

  async function yieldForStatusPaint(): Promise<void> {
    await new Promise<void>((resolve) => {
      let settled = false;
      let frame = 0;
      let timeout = 0;
      const settle = (): void => {
        if (settled) return;
        settled = true;
        cancelAnimationFrame(frame);
        window.clearTimeout(timeout);
        resolve();
      };
      frame = requestAnimationFrame(settle);
      timeout = window.setTimeout(settle, 50);
    });
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
      await yieldForStatusPaint();
      validateSources(state.files);
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
          const inputSource = inputSources.get(file.path);
          if (inputSource === undefined) throw new Error(t("responseSchema"));
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
    resultState.summary = providedSummary ?? {
      files: files.length,
      changed: files.filter((file) => file.changed).length,
      fixes: files.reduce((total, file) => total + file.fixes.length, 0),
      diagnostics: files.reduce((total, file) => total + file.diagnostics.length, 0),
      failed: files.filter((file) => file.error).length,
    };
    if (!state.selectedResult || !state.results.has(state.selectedResult)) {
      state.selectedResult = files[0]?.path ?? null;
    }
    renderSelectedResult();
    renderFileList();
    elements.results.hidden = false;
  }

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
        translate: (key: string) => t(key as Parameters<Translator>[0]),
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
          values
            ? t(key as Parameters<Translator>[0], values)
            : t(key as Parameters<Translator>[0]),
      },
    });
  }

  function activateTab(view: ResultView): void {
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

  function selectFormattedOutput(): void {
    const selection = window.getSelection();
    if (!selection || !formattedElement) return;
    const range = document.createRange();
    range.selectNodeContents(formattedElement);
    selection.removeAllRanges();
    selection.addRange(range);
  }

  async function copyCurrent(): Promise<void> {
    const result = selectedResult();
    if (!result || result.error || !result.stable) return;
    try {
      await navigator.clipboard.writeText(result.formatted);
      flashCopyLabel(t("copied"));
    } catch {
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
      .filter(
        (file) => !file.error && file.stable && state.files.get(file.path) === file.inputSource,
      )
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

  function refreshTranslation(): void {
    if (state.results.size > 0) renderRunResult();
    else resetCopyLabel();
  }

  function setFormattedElement(element: HTMLElement): void {
    formattedElement = element;
  }

  return {
    invalidateResults,
    runFormatter,
    refreshTranslation,
    setFormattedElement,
  };
}
