import { describe, expect, it } from "vitest";

import { errorText } from "./notify";

describe("errorText", () => {
  it("uses an Error's message", () => {
    expect(errorText(new Error("disk full"))).toBe("disk full");
  });

  it("passes a Tauri command's rejection through as text", () => {
    expect(errorText("Scryfall is down")).toBe("Scryfall is down");
  });
});
