import { describe, expect, it } from "vitest";

import { findOption, type Option } from "./options";

type Fruit = "apple" | "pear";
const FRUITS: Option<Fruit>[] = [
  { value: "apple", label: "Apple" },
  { value: "pear", label: "Pear" },
];

describe("findOption", () => {
  it("narrows a value that is one of the options", () => {
    expect(findOption(FRUITS, "pear")).toBe("pear");
  });

  it("returns undefined for anything else", () => {
    expect(findOption(FRUITS, "plum")).toBeUndefined();
    expect(findOption(FRUITS, "")).toBeUndefined();
    expect(findOption(FRUITS, null)).toBeUndefined();
    expect(findOption(FRUITS, undefined)).toBeUndefined();
  });
});
