import { afterEach, describe, expect, it, vi } from "vitest";

import { toaster } from "@mtg-art/ui/toaster";

import { errorText, showError, showInfo, showSuccess, showWarning } from "./notify";

describe("errorText", () => {
  it("uses an Error's message", () => {
    expect(errorText(new Error("disk full"))).toBe("disk full");
  });

  it("passes a Tauri command's rejection through as text", () => {
    expect(errorText("Scryfall is down")).toBe("Scryfall is down");
  });
});

/** Spies on the toasts made. */
function created() {
  return vi.spyOn(toaster, "create").mockReturnValue("toast");
}

describe("toasts", () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("shows a warning, with a button and the time to press it", () => {
    const create = created();
    const onClick = vi.fn<() => void>();
    showWarning("Card Anvil isn't installed", "Install it.", "send", {
      label: "Get Card Anvil",
      onClick,
    });
    expect(create).toHaveBeenCalledWith({
      id: "send",
      type: "warning",
      title: "Card Anvil isn't installed",
      description: "Install it.",
      action: { label: "Get Card Anvil", onClick },
      duration: 10_000,
    });
  });

  it("leaves out an id and a button not given", () => {
    const create = created();
    showWarning("Careful");
    expect(create).toHaveBeenCalledWith({
      type: "warning",
      title: "Careful",
      description: undefined,
    });
    showInfo("Note", "Something to know");
    expect(create).toHaveBeenLastCalledWith({
      type: "info",
      title: "Note",
      description: "Something to know",
    });
  });

  it("gives success and error toasts a button too", () => {
    const create = created();
    const action = { label: "Open", onClick: vi.fn<() => void>() };
    showSuccess("Sent 3 images", "send", "Card Anvil will ask where to put them.", action);
    expect(create).toHaveBeenLastCalledWith(
      expect.objectContaining({
        id: "send",
        type: "success",
        description: "Card Anvil will ask where to put them.",
        action,
      }),
    );
    showError("Couldn't send", "Disk full", undefined, action);
    expect(create).toHaveBeenLastCalledWith(
      expect.objectContaining({ type: "error", description: "Disk full", action }),
    );
  });
});
