import { toaster } from "@/components/ui/toaster";

/** A Tauri command's error as text: commands reject with the Rust error's message. */
export function errorText(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function showError(title: string, error: unknown) {
  toaster.create({ type: "error", title, description: errorText(error) });
}

export function showSuccess(title: string) {
  toaster.create({ type: "success", title });
}
