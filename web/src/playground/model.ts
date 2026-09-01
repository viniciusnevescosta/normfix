import type { SourceEditor } from "../editor";
import type { BrowserFileResult } from "../formatter-response";
import type { Locale, MessageKey } from "../i18n";
import type { OfflineState, OfflineSupport } from "../offline/pwa";
import type { Appearance, ThemePreference } from "../theme";

export const PROJECT_STORAGE_KEY = "normfix.project.v1";

export const SAMPLE = `#include <unistd.h>

int main(void)
{
    if (write(1, "normfix\\n", 8) > 0) { return (0); }
    else { return (1); }
}
`;

export type RuntimeState = "loading" | "ready" | "error";
export type FormatProject = (request: string) => string;
export type Translator = (
  key: MessageKey,
  values?: Readonly<Record<string, string | number>>,
) => string;
export type PluralTranslator = (base: string, count: number) => string;

export interface ResultRecord extends BrowserFileResult {
  inputSource: string;
}

export interface WasmModule {
  default: () => Promise<unknown>;
  formatProject: FormatProject;
}

export interface AppState {
  files: Map<string, string>;
  /** Explicit project directories, including directories with no files. */
  folders: Set<string>;
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

export function createAppState(locale: Locale, theme: ThemePreference): AppState {
  return {
    files: new Map<string, string>([["main.c", SAMPLE]]),
    folders: new Set<string>(),
    selected: "main.c",
    results: new Map<string, ResultRecord>(),
    selectedResult: null,
    formatter: null,
    editor: null,
    running: false,
    importing: false,
    revision: 0,
    identityEmail: null,
    locale,
    offlineState: "unsupported",
    offlineSupport: null,
    theme,
    appearance: "dark",
    unsupported: new Set<string>(),
  };
}
