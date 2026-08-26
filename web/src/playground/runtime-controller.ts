import type { MessageKey } from "../i18n";
import type { PlaygroundElements } from "../dom";
import { statusState } from "../tree-state.svelte";
import type { AppState, RuntimeState, Translator, WasmModule } from "./model";

export interface RuntimeController {
  refreshRunControl(): void;
  setRuntime(stateName: RuntimeState, label: string): void;
  setRuntimeMessage(stateName: RuntimeState, key: MessageKey): void;
  refreshTranslation(): void;
  renderOfflineStatus(): void;
  loadFormatter(): Promise<void>;
}

export function createRuntimeController(
  state: AppState,
  elements: PlaygroundElements,
  t: Translator,
): RuntimeController {
  let runtimeMessageKey: MessageKey | null = null;

  function refreshRunControl(): void {
    elements.run.disabled =
      state.formatter === null ||
      state.editor === null ||
      state.running ||
      state.importing ||
      state.files.size === 0;
  }

  function setRuntime(stateName: RuntimeState, label: string): void {
    statusState.runtime = stateName;
    statusState.runtimeLabel = label;
    runtimeMessageKey = null;
  }

  function setRuntimeMessage(stateName: RuntimeState, key: MessageKey): void {
    setRuntime(stateName, t(key));
    runtimeMessageKey = key;
  }

  function refreshTranslation(): void {
    if (runtimeMessageKey) statusState.runtimeLabel = t(runtimeMessageKey);
  }

  function renderOfflineStatus(): void {
    statusState.offline = state.offlineState;
  }

  async function loadFormatter(): Promise<void> {
    try {
      const module = (await import("../../pkg/normfix_wasm.js")) as WasmModule;
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

  return {
    refreshRunControl,
    setRuntime,
    setRuntimeMessage,
    refreshTranslation,
    renderOfflineStatus,
    loadFormatter,
  };
}
