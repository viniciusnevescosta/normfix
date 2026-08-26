import { loadElements } from "./dom";
import { createSourceEditor } from "./editor";
import {
  type Locale,
  type MessageKey,
  SUPPORTED_LOCALES,
  translate,
  translatePlural,
} from "./i18n";
import { setLocale } from "./i18n-state.svelte";
import { localizeDocument, readInitialLocale, storeLocale } from "./localization";
import { mountStaticUi } from "./mount-ui";
import { startOfflineSupport } from "./offline/pwa";
import { createIdentityController } from "./playground/identity-controller";
import { createImportController } from "./playground/import-controller";
import { createAppState } from "./playground/model";
import { createPersistenceController } from "./playground/persistence-controller";
import { createProjectController } from "./playground/project-controller";
import { createResultsController } from "./playground/results-controller";
import { createRuntimeController } from "./playground/runtime-controller";
import {
  type ThemePreference,
  applyThemePreference,
  isThemePreference,
  readStoredThemePreference,
  storeThemePreference,
  watchSystemAppearance,
} from "./theme";
import { chromeState, headerState, statusState } from "./tree-state.svelte";

const state = createAppState(readInitialLocale(), readStoredThemePreference());
const elements = loadElements();

function t(key: MessageKey, values: Readonly<Record<string, string | number>> = {}): string {
  return translate(state.locale, key, values);
}

function tPlural(base: string, count: number): string {
  return translatePlural(state.locale, base, count);
}

const runtime = createRuntimeController(state, elements, t);
let persistence: ReturnType<typeof createPersistenceController>;
let project: ReturnType<typeof createProjectController>;
let results: ReturnType<typeof createResultsController>;

project = createProjectController({
  state,
  elements,
  translate: t,
  invalidateResults: () => results.invalidateResults(),
  refreshRunControl: runtime.refreshRunControl,
});
results = createResultsController({
  state,
  elements,
  translate: t,
  translatePlural: tPlural,
  setRuntime: runtime.setRuntime,
  setRuntimeMessage: runtime.setRuntimeMessage,
  refreshRunControl: runtime.refreshRunControl,
  scheduleSave: () => persistence.scheduleSave(),
  syncEditor: project.syncEditor,
  validateSources: project.validateSources,
  renderFileList: project.renderFileList,
  selectFile: project.selectFile,
});
persistence = createPersistenceController({
  state,
  elements,
  translate: t,
  refreshRunControl: runtime.refreshRunControl,
  invalidateResults: results.invalidateResults,
  selectFile: project.selectFile,
});
const imports = createImportController({
  state,
  elements,
  translate: t,
  translatePlural: tPlural,
  setRuntime: runtime.setRuntime,
  refreshRunControl: runtime.refreshRunControl,
  syncEditor: project.syncEditor,
  normalizePath: project.normalizePath,
  validateSources: project.validateSources,
  invalidateResults: results.invalidateResults,
  selectFile: project.selectFile,
  renderFileList: project.renderFileList,
});
const identity = createIdentityController(state, t, results.invalidateResults);

function applyTranslations(): void {
  chromeState.locale = state.locale;
  chromeState.theme = state.theme;
  runtime.refreshTranslation();
  runtime.renderOfflineStatus();
  elements.addFile.setAttribute("aria-label", t("addFile"));
  elements.addFolder.setAttribute("aria-label", t("addFolder"));
  chromeState.docsHref = localizeDocument(state.locale, elements, (key) => t(key)).docsHref;
  project.updateEditorMeta();
  results.refreshTranslation();
}

function changeLocale(locale: Locale): void {
  state.locale = locale;
  setLocale(locale);
  storeLocale(locale);
  applyTranslations();
}

function changeTheme(preference: ThemePreference): void {
  state.theme = preference;
  state.appearance = applyThemePreference(preference);
  state.editor?.setAppearance(state.appearance);
  storeThemePreference(preference);
  chromeState.theme = preference;
}

watchSystemAppearance(() => {
  if (state.theme === "system") changeTheme("system");
});

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
  onSaveIdentity: identity.save,
  onForgetIdentity: identity.forget,
  onOfflineUpdate: () => state.offlineSupport?.applyUpdate(),
  onFormattedElement: results.setFormattedElement,
});

elements.run.addEventListener("click", () => {
  void results.runFormatter();
});
elements.removeFile.addEventListener("click", project.removeSelected);
elements.addFile.addEventListener("click", () => project.openDraft("file"));
elements.addFolder.addEventListener("click", () => project.openDraft("folder"));
imports.attach();
persistence.attach();

async function initialize(): Promise<void> {
  imports.setControls(true);
  setLocale(state.locale);
  runtime.setRuntimeMessage("loading", "loadingFormatter");
  state.appearance = applyThemePreference(state.theme);
  applyTranslations();
  identity.load();
  persistence.restoreProject();
  state.offlineSupport = startOfflineSupport({
    onState: (offlineState) => {
      state.offlineState = offlineState;
      runtime.renderOfflineStatus();
    },
    onConnectivity: (online) => {
      statusState.online = online;
    },
  });
  const formatterPromise = runtime.loadFormatter();
  const opening = state.selected ?? "main.c";
  headerState.path = state.selected;
  try {
    state.editor = await createSourceEditor(
      elements.editorContainer,
      elements.fallbackEditor,
      opening,
      state.files.get(opening) ?? "",
      state.appearance,
      {
        onChange: () => {
          project.syncEditor();
          project.updateEditorMeta();
        },
        onRun: () => {
          void results.runFormatter();
        },
      },
    );
  } finally {
    imports.setControls(false);
  }
  project.renderFileList();
  project.updateEditorMeta();
  await formatterPromise;
}

void initialize().catch((error: unknown) => {
  console.error(error);
  runtime.setRuntime("error", error instanceof Error ? error.message : String(error));
});
