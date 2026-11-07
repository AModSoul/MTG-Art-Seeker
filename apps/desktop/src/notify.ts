import { toaster } from "@mtg-art/ui/toaster";

/** A Tauri command's error as text: commands reject with the Rust error's message. */
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** A toast can be given an `id`: another with the same one replaces it, so a
 * loading toast can turn into its result. */
export function showError(title: string, error: unknown, id?: string) {
  toaster.create({ ...withId(id), type: "error", title, description: errorText(error) });
}

export function showSuccess(title: string, id?: string) {
  toaster.create({ ...withId(id), type: "success", title });
}

export function showInfo(title: string, description?: string, id?: string) {
  toaster.create({ ...withId(id), type: "info", title, description });
}

/** A toast with a spinner, until another with the same `id` replaces it. */
export function showLoading(title: string, id: string) {
  toaster.create({ id, type: "loading", title });
}

/** The toast's `id`, left out when there's none: zag rejects `id: undefined`. */
function withId(id: string | undefined): { id?: string } {
  return id === undefined ? {} : { id };
}
