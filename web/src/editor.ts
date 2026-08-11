import type * as Monaco from "monaco-editor/editor.js";
import EditorWorker from "monaco-editor/editor/editor.worker.js?worker";

export interface SourceEditor {
  getValue(): string;
  setFile(path: string, source: string): void;
  removeFile(path: string): void;
  focus(): void;
  usingMonaco: boolean;
}

interface EditorCallbacks {
  onChange: () => void;
  onRun: () => void;
}

interface MonacoEnvironmentShape {
  getWorker(_moduleId: string, _label: string): Worker;
}

function languageForPath(path: string): string {
  const filename = path.split("/").at(-1)?.toLowerCase() ?? "";
  if (filename === "makefile") return "makefile";
  if (filename.endsWith(".md")) return "markdown";
  return "c";
}

function monacoSupported(): boolean {
  return window.matchMedia("(min-width: 761px) and (pointer: fine)").matches;
}

function createFallbackEditor(
  monacoContainer: HTMLElement,
  fallback: HTMLTextAreaElement,
  initialSource: string,
  callbacks: EditorCallbacks,
): SourceEditor {
  monacoContainer.hidden = true;
  fallback.hidden = false;
  fallback.value = initialSource;
  fallback.addEventListener("input", callbacks.onChange);
  fallback.addEventListener("keydown", (event) => {
    if ((event.metaKey || event.ctrlKey) && event.key === "Enter") {
      event.preventDefault();
      callbacks.onRun();
    }
  });
  return {
    usingMonaco: false,
    getValue: () => fallback.value,
    setFile: (_path, source) => {
      fallback.value = source;
    },
    removeFile: () => undefined,
    focus: () => fallback.focus(),
  };
}

export async function createSourceEditor(
  monacoContainer: HTMLElement,
  fallback: HTMLTextAreaElement,
  initialPath: string,
  initialSource: string,
  callbacks: EditorCallbacks,
): Promise<SourceEditor> {
  if (!monacoSupported()) {
    return createFallbackEditor(monacoContainer, fallback, initialSource, callbacks);
  }
  try {
    return await createMonacoEditor(monacoContainer, fallback, initialPath, initialSource, callbacks);
  } catch (error) {
    // Monaco is a convenience, not the product. It is a large dynamic import,
    // so it is the first thing to fail on a lost connection or a cold offline
    // start — and losing it must cost syntax highlighting, not the ability to
    // format code at all.
    console.error(error);
    return createFallbackEditor(monacoContainer, fallback, initialSource, callbacks);
  }
}

async function createMonacoEditor(
  monacoContainer: HTMLElement,
  fallback: HTMLTextAreaElement,
  initialPath: string,
  initialSource: string,
  callbacks: EditorCallbacks,
): Promise<SourceEditor> {
  fallback.hidden = true;
  monacoContainer.hidden = false;
  (globalThis as typeof globalThis & { MonacoEnvironment?: MonacoEnvironmentShape })
    .MonacoEnvironment = {
    getWorker: () => new EditorWorker(),
  };
  const monaco = await loadMonaco();
  assertRequiredLanguages(monaco);
  registerMakefile(monaco);
  monaco.editor.defineTheme("normfix", {
    base: "vs-dark",
    inherit: true,
    rules: [
      { token: "comment", foreground: "7d877d" },
      { token: "keyword", foreground: "8fc7a8" },
      { token: "string", foreground: "d5b56d" },
      { token: "number", foreground: "86aeca" },
    ],
    colors: {
      "editor.background": "#121412",
      "editor.foreground": "#e5e9e5",
      "editorLineNumber.foreground": "#596159",
      "editorLineNumber.activeForeground": "#b7c0b7",
      "editor.selectionBackground": "#365143",
      "editor.inactiveSelectionBackground": "#293a30",
      "editorCursor.foreground": "#8fc7a8",
      "editorIndentGuide.background1": "#272c27",
      "editorIndentGuide.activeBackground1": "#596159",
      "editor.lineHighlightBackground": "#171a17",
      "editorWidget.background": "#171a17",
      "editorWidget.border": "#596159",
    },
  });

  const models = new Map<string, Monaco.editor.ITextModel>();
  const createModel = (path: string, source: string): Monaco.editor.ITextModel => {
    const uri = monaco.Uri.from({ scheme: "file", path: `/${path}` });
    const model = monaco.editor.createModel(source, languageForPath(path), uri);
    models.set(path, model);
    return model;
  };
  const instance = monaco.editor.create(monacoContainer, {
    model: createModel(initialPath, initialSource),
    theme: "normfix",
    automaticLayout: true,
    lineNumbers: "on",
    lineNumbersMinChars: 3,
    glyphMargin: false,
    minimap: { enabled: false },
    fontFamily: '"SFMono-Regular", "Cascadia Mono", "Liberation Mono", Menlo, monospace',
    fontSize: 12,
    lineHeight: 19,
    tabSize: 4,
    insertSpaces: false,
    detectIndentation: false,
    renderWhitespace: "selection",
    renderControlCharacters: true,
    bracketPairColorization: { enabled: true },
    guides: { bracketPairs: true, indentation: true },
    folding: true,
    wordWrap: "off",
    scrollBeyondLastLine: false,
    smoothScrolling: false,
    contextmenu: true,
    quickSuggestions: true,
    suggestOnTriggerCharacters: true,
    accessibilitySupport: "auto",
    padding: { top: 10, bottom: 10 },
  });
  instance.onDidChangeModelContent(callbacks.onChange);
  instance.addCommand(monaco.KeyMod.CtrlCmd | monaco.KeyCode.Enter, callbacks.onRun);

  return {
    usingMonaco: true,
    getValue: () => instance.getValue(),
    setFile: (path, source) => {
      let model = models.get(path);
      if (!model || model.isDisposed()) model = createModel(path, source);
      if (model.getValue() !== source) model.setValue(source);
      instance.setModel(model);
    },
    removeFile: (path) => {
      const model = models.get(path);
      models.delete(path);
      model?.dispose();
    },
    focus: () => instance.focus(),
  };
}

async function loadMonaco(): Promise<typeof Monaco> {
  const [monaco] = await Promise.all([
    import("monaco-editor/editor.js"),
    import("monaco-editor/languages/definitions/cpp/register.js"),
    import("monaco-editor/languages/definitions/markdown/register.js"),
    import("monaco-editor/features/codeEditor/register.js"),
    import("monaco-editor/features/find/register.js"),
    import("monaco-editor/features/folding/register.js"),
    import("monaco-editor/features/bracketMatching/register.js"),
    import("monaco-editor/features/contextmenu/register.js"),
    import("monaco-editor/features/multicursor/register.js"),
    import("monaco-editor/features/clipboard/register.js"),
    import("monaco-editor/features/comment/register.js"),
    import("monaco-editor/features/indentation/register.js"),
    import("monaco-editor/features/linesOperations/register.js"),
    import("monaco-editor/features/wordOperations/register.js"),
    import("monaco-editor/features/tokenization/register.js"),
    import("monaco-editor/editor/contrib/suggest/browser/suggestController.js"),
    import("monaco-editor/features/gotoLine/register.js"),
  ]);
  return monaco;
}

function assertRequiredLanguages(monaco: typeof Monaco): void {
  const registered = new Set(monaco.languages.getLanguages().map((language) => language.id));
  const missing = ["c", "cpp", "markdown"].filter((language) => !registered.has(language));
  if (missing.length > 0) {
    throw new Error(`Monaco language registration failed: ${missing.join(", ")}`);
  }
}

function registerMakefile(monaco: typeof Monaco): void {
  if (!monaco.languages.getLanguages().some((language) => language.id === "makefile")) {
    monaco.languages.register({
      id: "makefile",
      aliases: ["Makefile", "makefile"],
      filenames: ["Makefile", "makefile"],
    });
  }
  monaco.languages.setLanguageConfiguration("makefile", {
    comments: { lineComment: "#" },
    brackets: [
      ["(", ")"],
      ["{", "}"],
    ],
    autoClosingPairs: [
      { open: "(", close: ")" },
      { open: "{", close: "}" },
      { open: '"', close: '"' },
      { open: "'", close: "'" },
    ],
  });
  monaco.languages.setMonarchTokensProvider("makefile", {
    tokenizer: {
      root: [
        [/^\t.*$/, "string"],
        [/#[^\n]*$/, "comment"],
        [/\$[({][^)}]+[)}]/, "variable"],
        [/^[A-Za-z_][\w.-]*(?=\s*[:+?]?=)/, "variable.predefined"],
        [/^[^\s:#=]+(?=\s*:)/, "type.identifier"],
        [/:{1,2}|\+=|\?=|:=|=/, "operator"],
        [/\b(?:include|ifdef|ifndef|ifeq|ifneq|else|endif|define|endef|export|unexport|override|private|vpath)\b/, "keyword"],
        [/"[^"\\]*(?:\\.[^"\\]*)*"/, "string"],
        [/'[^'\\]*(?:\\.[^'\\]*)*'/, "string"],
      ],
    },
  });
}
