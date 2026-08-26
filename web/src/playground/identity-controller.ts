import {
  chooseIdentity,
  hasStoredIdentity,
  loadBrowserIdentity,
  removeStoredIdentity,
} from "../identity";
import { identityState } from "../tree-state.svelte";
import type { AppState, Translator } from "./model";

export interface IdentityController {
  load(): void;
  save(typed: string, remember: boolean): void;
  forget(): void;
}

export function createIdentityController(
  state: AppState,
  t: Translator,
  invalidateResults: () => void,
): IdentityController {
  function render(): void {
    identityState.stored = hasStoredIdentity();
    identityState.email = state.identityEmail ?? "";
  }

  function setStatus(key: Parameters<Translator>[0], invalid = false): void {
    identityState.status = t(key);
    identityState.invalid = invalid;
  }

  function load(): void {
    state.identityEmail = loadBrowserIdentity();
    render();
  }

  function save(typed: string, remember: boolean): void {
    const choice = chooseIdentity(typed, remember);
    if (choice.email === null) {
      setStatus("invalidIdentity", true);
      return;
    }
    state.identityEmail = choice.email;
    state.revision += 1;
    invalidateResults();
    setStatus(
      choice.outcome === "saved"
        ? "identitySaved"
        : choice.outcome === "storage_unavailable"
          ? "storageUnavailable"
          : "identitySession",
    );
    render();
  }

  function forget(): void {
    state.identityEmail = null;
    state.revision += 1;
    invalidateResults();
    removeStoredIdentity();
    setStatus("identityForgotten");
    render();
  }

  return { load, save, forget };
}
