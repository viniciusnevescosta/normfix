export function requiredElement<T extends Element>(selector: string): T {
  const element = document.querySelector<T>(selector);
  if (!element) throw new Error(`Required element is missing: ${selector}`);
  return element;
}

/** Resolves the static shell once, before controllers attach any behavior. */
export function loadElements() {
  return {
    fileList: requiredElement<HTMLElement>("#file-list"),
    filePicker: requiredElement<HTMLInputElement>("#file-picker"),
    dropOverlay: requiredElement<HTMLElement>("#drop-overlay"),
    statusBadges: requiredElement<HTMLElement>("#status-badges"),
    addFile: requiredElement<HTMLButtonElement>("#add-file"),
    addFolder: requiredElement<HTMLButtonElement>("#add-folder"),
    removeFile: requiredElement<HTMLButtonElement>("#remove-file"),
    editorContainer: requiredElement<HTMLElement>("#monaco-editor"),
    fallbackEditor: requiredElement<HTMLTextAreaElement>("#fallback-editor"),
    identityPanel: requiredElement<HTMLElement>("#identity-panel"),
    editorNotice: requiredElement<HTMLElement>("#editor-notice"),
    editorHeader: requiredElement<HTMLElement>("#editor-header"),
    topBar: requiredElement<HTMLElement>("#top-bar"),
    confirmDelete: requiredElement<HTMLElement>("#confirm-delete"),
    restoreNotice: requiredElement<HTMLElement>("#restore-notice"),
    discardRestore: requiredElement<HTMLButtonElement>("#discard-restore"),
    run: requiredElement<HTMLButtonElement>("#run"),
    results: requiredElement<HTMLElement>("#results"),
    resultSummary: requiredElement<HTMLElement>("#result-summary"),
    formattedOutput: requiredElement<HTMLElement>("#formatted-output"),
    diffOutput: requiredElement<HTMLElement>("#diff-output"),
    diagnosticsView: requiredElement<HTMLElement>("#diagnostics-view"),
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
}

export type PlaygroundElements = ReturnType<typeof loadElements>;
