import { toaster } from "@mtg-art/ui/toaster";

/** A Tauri command's error as text: commands reject with the Rust error's message. */
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/** A button on a toast, for what the user can do about it. */
export interface ToastAction {
  label: string;
  onClick: () => void;
}

/** A toast can be given an `id`: another with the same one replaces it, so a
 * loading toast can turn into its result. */
export function showError(title: string, error: unknown, id?: string, action?: ToastAction) {
  toaster.create({
    ...withId(id),
    ...withAction(action),
    type: "error",
    title,
    description: errorText(error),
  });
}

export function showSuccess(
  title: string,
  id?: string,
  description?: string,
  action?: ToastAction,
) {
  toaster.create({ ...withId(id), ...withAction(action), type: "success", title, description });
}

export function showInfo(title: string, description?: string, id?: string, action?: ToastAction) {
  toaster.create({ ...withId(id), ...withAction(action), type: "info", title, description });
}

/** Something didn't happen, but nothing went wrong: the user has something to do. */
export function showWarning(
  title: string,
  description?: string,
  id?: string,
  action?: ToastAction,
) {
  toaster.create({ ...withId(id), ...withAction(action), type: "warning", title, description });
}

/** A toast with a spinner, until another with the same `id` replaces it. */
export function showLoading(title: string, id: string) {
  toaster.create({ id, type: "loading", title });
}

/** The toast's `id`, left out when there's none: zag rejects `id: undefined`. */
function withId(id: string | undefined): { id?: string } {
  return id === undefined ? {} : { id };
}

/** How long a toast with a button stays, so there's time to read it and press the button. */
const ACTION_DURATION_MS = 10_000;

/** The toast's button, and the time to use it; nothing when there's none. */
function withAction(action: ToastAction | undefined): { action?: ToastAction; duration?: number } {
  return action === undefined ? {} : { action, duration: ACTION_DURATION_MS };
}
