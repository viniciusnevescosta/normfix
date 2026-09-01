// Naming a new entry where it will live.
//
// An editor does not ask for a filename in a modal. It puts the row in the list
// and lets you type over it: Enter commits, Escape abandons, clicking away
// abandons. This module is that row and nothing else — it is handed what to do
// with a finished name, so the page's state stays in the page and this stays
// testable without one.

/** What the row needs from the page around it. */
export interface DraftRowOptions {
  /** Where the row is appended. */
  container: Element;
  /** The kind of entry being named. */
  kind: "file" | "folder";
  /** Prefix already chosen, ending in `/` when present. */
  prefix?: string;
  /** Accessible name for the input, already translated. */
  label: string;
  /**
   * Commits a name. Throwing keeps the row open with the message shown, which
   * is the point: a refused name is corrected rather than retyped.
   */
  create: (path: string) => void;
  /** Called once the row is gone, whether it committed or not. */
  onClose?: () => void;
}

/**
 * Opens a row whose name is waiting to be typed.
 *
 * Files and folders are both real project entries. In particular, committing
 * a folder ends this interaction: creating a directory must not force the
 * reader to invent a file before they can continue.
 */
export function openDraftRow(options: DraftRowOptions): void {
  const { container, kind, prefix = "", label, create, onClose } = options;
  const document = container.ownerDocument;
  const row = document.createElement("div");
  row.className = "file-item file-draft";

  const dot = document.createElement("span");
  dot.className = "file-dot";
  dot.setAttribute("aria-hidden", "true");
  const input = document.createElement("input");
  input.className = "file-name";
  input.setAttribute("aria-label", label);
  input.placeholder = kind === "file" ? "new_file.c" : "src";
  const error = document.createElement("span");
  error.className = "file-draft-error";
  row.append(dot, input, error);
  container.append(row);
  input.focus();

  let settled = false;
  const close = (): void => {
    if (settled) return;
    settled = true;
    row.remove();
    onClose?.();
  };

  input.addEventListener("keydown", (event) => {
    const key = (event as KeyboardEvent).key;
    if (key === "Escape") {
      close();
      return;
    }
    if (key !== "Enter") return;
    event.preventDefault();
    const typed = input.value.trim();
    if (typed.length === 0) {
      close();
      return;
    }
    try {
      create(`${prefix}${kind === "folder" ? typed.replace(/\/+$/, "") : typed}`);
      close();
    } catch (failure) {
      error.textContent = failure instanceof Error ? failure.message : String(failure);
      input.select();
    }
  });

  input.addEventListener("blur", () => {
    // Clicking away abandons the row, but not while it is saying why a name was
    // refused: the reader is reading it, and closing would take back what they
    // typed along with the explanation.
    if (error.textContent === "") close();
  });
}
