import { canonicalIdentityEmail } from "./project/files";

export const IDENTITY_STORAGE_KEY = "normfix.identity.v1";

export type IdentityOutcome = "invalid" | "session" | "saved" | "storage_unavailable";

export interface IdentityChoice {
  email: string | null;
  outcome: IdentityOutcome;
}

/** Reads and repairs the one identity value the playground owns. */
export function loadBrowserIdentity(): string | null {
  try {
    const stored = localStorage.getItem(IDENTITY_STORAGE_KEY);
    const canonical = stored ? canonicalIdentityEmail(stored) : null;
    if (stored && canonical === null) localStorage.removeItem(IDENTITY_STORAGE_KEY);
    return canonical;
  } catch {
    return null;
  }
}

export function hasStoredIdentity(): boolean {
  try {
    return canonicalIdentityEmail(localStorage.getItem(IDENTITY_STORAGE_KEY) ?? "") !== null;
  } catch {
    return false;
  }
}

/** Applies the explicit persistence choice; unchecked always clears browser storage. */
export function chooseIdentity(value: string, remember: boolean): IdentityChoice {
  const email = canonicalIdentityEmail(value);
  if (email === null) return { email: null, outcome: "invalid" };
  if (!remember) {
    try {
      localStorage.removeItem(IDENTITY_STORAGE_KEY);
    } catch {
      // Session use needs no storage.
    }
    return { email, outcome: "session" };
  }
  try {
    localStorage.setItem(IDENTITY_STORAGE_KEY, email);
    return { email, outcome: "saved" };
  } catch {
    return { email, outcome: "storage_unavailable" };
  }
}

export function removeStoredIdentity(): void {
  try {
    localStorage.removeItem(IDENTITY_STORAGE_KEY);
  } catch {
    // The in-memory identity is cleared by the caller either way.
  }
}
