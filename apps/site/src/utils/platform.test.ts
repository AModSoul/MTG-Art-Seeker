import { describe, expect, it } from "vitest";

import { detectArch, detectOs } from "./platform";

const UA = {
  windows:
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36",
  mac: "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Safari/605.1.15",
  linux: "Mozilla/5.0 (X11; Linux x86_64; rv:140.0) Gecko/20100101 Firefox/140.0",
  linuxArm: "Mozilla/5.0 (X11; Linux aarch64; rv:140.0) Gecko/20100101 Firefox/140.0",
  android:
    "Mozilla/5.0 (Linux; Android 15; Pixel 9) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Mobile Safari/537.36",
  iphone:
    "Mozilla/5.0 (iPhone; CPU iPhone OS 18_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.0 Mobile/15E148 Safari/604.1",
};

describe("detectOs", () => {
  it("reads client hints first", () => {
    expect(detectOs({ userAgent: UA.mac, userAgentData: { platform: "Windows" } })).toBe("windows");
    expect(detectOs({ userAgent: "", userAgentData: { platform: "Linux" } })).toBe("linux");
    expect(detectOs({ userAgent: "", userAgentData: { platform: "Android" } })).toBeNull();
  });

  it("falls back to the user agent", () => {
    expect(detectOs({ userAgent: UA.windows })).toBe("windows");
    expect(detectOs({ userAgent: UA.mac })).toBe("macos");
    expect(detectOs({ userAgent: UA.linux })).toBe("linux");
  });

  it("doesn't take phones and tablets for desktops", () => {
    expect(detectOs({ userAgent: UA.android })).toBeNull();
    expect(detectOs({ userAgent: UA.iphone })).toBeNull();
    // iPadOS asks for desktop sites with a Mac user agent.
    expect(detectOs({ userAgent: UA.mac, maxTouchPoints: 5 })).toBeNull();
  });
});

const hints = (architecture: string) => ({
  platform: "macOS",
  getHighEntropyValues: async () => ({ architecture }),
});

describe("detectArch", () => {
  it("reads client hints", async () => {
    await expect(detectArch({ userAgent: UA.mac, userAgentData: hints("arm") })).resolves.toBe(
      "arm64",
    );
    await expect(detectArch({ userAgent: UA.mac, userAgentData: hints("x86") })).resolves.toBe(
      "x64",
    );
  });

  it("never trusts a Mac's user agent", async () => {
    await expect(detectArch({ userAgent: UA.mac })).resolves.toBeNull();
  });

  it("reads other systems' user agents", async () => {
    await expect(detectArch({ userAgent: UA.windows })).resolves.toBe("x64");
    await expect(detectArch({ userAgent: UA.linux })).resolves.toBe("x64");
    await expect(detectArch({ userAgent: UA.linuxArm })).resolves.toBe("arm64");
  });

  it("falls back to the user agent when hints fail", async () => {
    const failing = {
      platform: "Windows",
      getHighEntropyValues: () => Promise.reject(new Error("denied")),
    };
    await expect(detectArch({ userAgent: UA.windows, userAgentData: failing })).resolves.toBe(
      "x64",
    );
  });
});
