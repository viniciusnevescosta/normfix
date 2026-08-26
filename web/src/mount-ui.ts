import { mount } from "svelte";
import CodeView from "./components/CodeView.svelte";
import ConfirmDialog from "./components/ConfirmDialog.svelte";
import DropOverlay from "./components/DropOverlay.svelte";
import EditorHeader from "./components/EditorHeader.svelte";
import EditorNotice from "./components/EditorNotice.svelte";
import IdentityPanel from "./components/IdentityPanel.svelte";
import StatusBadges from "./components/StatusBadges.svelte";
import TopBar from "./components/TopBar.svelte";
import type { Locale, MessageKey } from "./i18n";
import {
  chromeState,
  codeState,
  confirmState,
  dragState,
  editorState,
  headerState,
  identityState,
  statusState,
} from "./tree-state.svelte";

export interface StaticMountElements {
  topBar: HTMLElement;
  formattedOutput: HTMLElement;
  diffOutput: HTMLElement;
  editorHeader: HTMLElement;
  confirmDelete: HTMLElement;
  statusBadges: HTMLElement;
  dropOverlay: HTMLElement;
  editorNotice: HTMLElement;
  identityPanel: HTMLElement;
}

export interface StaticMountOptions {
  elements: StaticMountElements;
  locale: () => Locale;
  translate: (key: MessageKey, values?: Readonly<Record<string, string | number>>) => string;
  onLocale: (locale: string) => void;
  onTheme: (theme: string) => void;
  onSaveIdentity: (email: string, remember: boolean) => void;
  onForgetIdentity: () => void;
  onOfflineUpdate: () => void;
  onFormattedElement: (element: HTMLElement) => void;
}

/** Mounts the state-only components once; controllers only update their shared state. */
export function mountStaticUi(options: StaticMountOptions): void {
  const { elements } = options;
  mount(TopBar, {
    target: elements.topBar,
    props: {
      get locale() {
        return chromeState.locale;
      },
      get theme() {
        return chromeState.theme;
      },
      get docsHref() {
        return chromeState.docsHref;
      },
      onLocale: options.onLocale,
      onTheme: options.onTheme,
    },
  });

  mount(CodeView, {
    target: elements.formattedOutput,
    props: {
      get text() {
        return codeState.formatted;
      },
      bind: options.onFormattedElement,
    },
  });
  mount(CodeView, {
    target: elements.diffOutput,
    props: {
      get text() {
        return codeState.diff;
      },
    },
  });

  mount(EditorHeader, {
    target: elements.editorHeader,
    props: {
      get path() {
        return headerState.path;
      },
      get lines() {
        return headerState.lines;
      },
      get bytes() {
        return headerState.bytes;
      },
      measure: (lines: number, bytes: number) =>
        options.translate("linesBytes", {
          lines,
          bytes: bytes.toLocaleString(options.locale()),
        }),
      get label() {
        return options.translate("input");
      },
    },
  });

  const cancelDelete = (): void => {
    confirmState.request = null;
    confirmState.accept = null;
  };
  mount(ConfirmDialog, {
    target: elements.confirmDelete,
    props: {
      get request() {
        return confirmState.request;
      },
      onConfirm: () => {
        const accept = confirmState.accept;
        cancelDelete();
        accept?.();
      },
      onCancel: cancelDelete,
    },
  });

  mount(StatusBadges, {
    target: elements.statusBadges,
    props: {
      get runtime() {
        return statusState.runtime;
      },
      get runtimeLabel() {
        return statusState.runtimeLabel;
      },
      get offline() {
        return statusState.offline;
      },
      get online() {
        return statusState.online;
      },
      onUpdate: options.onOfflineUpdate,
    },
  });
  mount(DropOverlay, {
    target: elements.dropOverlay,
    props: {
      get active() {
        return dragState.active;
      },
    },
  });
  mount(EditorNotice, {
    target: elements.editorNotice,
    props: {
      get notice() {
        return editorState.notice;
      },
    },
  });
  mount(IdentityPanel, {
    target: elements.identityPanel,
    props: {
      get email() {
        return identityState.email;
      },
      get stored() {
        return identityState.stored;
      },
      get status() {
        return identityState.status;
      },
      get invalid() {
        return identityState.invalid;
      },
      onSave: options.onSaveIdentity,
      onForget: options.onForgetIdentity,
    },
  });
}
