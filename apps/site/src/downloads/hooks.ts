import { useEffect, useState } from "react";

import type { ReleaseState } from "@/components/DownloadPanel";
import { detectArch, detectOs } from "@/utils/platform";

import type { Arch, Os } from "./artifacts";
import { fetchLatestRelease } from "./github";
import { readReleaseSnapshot } from "./snapshot";

/**
 * The latest release: the build's snapshot at once when there is one, then whatever GitHub says
 * now. A failed refresh keeps the snapshot rather than showing an error.
 */
export function useLatestRelease(): ReleaseState {
  const [state, setState] = useState<ReleaseState>(() => {
    const snapshot = readReleaseSnapshot();
    return snapshot ? { status: "ready", release: snapshot } : { status: "loading" };
  });

  useEffect(() => {
    const controller = new AbortController();
    fetchLatestRelease({ signal: controller.signal }).then(
      (release) => setState({ status: "ready", release }),
      (error: unknown) => {
        if (controller.signal.aborted) return;
        const message = error instanceof Error ? error.message : "Something went wrong.";
        setState((current) =>
          current.status === "ready" ? current : { status: "error", message },
        );
      },
    );
    return () => controller.abort();
  }, []);

  return state;
}

/** The visitor's system and processor, as far as the browser says. Nothing during prerender. */
export function useVisitorPlatform(): { os: Os | null; arch: Arch | null } {
  const [os] = useState(() => (typeof window === "undefined" ? null : detectOs(navigator)));
  const [arch, setArch] = useState<Arch | null>(null);

  useEffect(() => {
    let current = true;
    void detectArch(navigator).then((detected) => {
      if (current) setArch(detected);
    });
    return () => {
      current = false;
    };
  }, []);

  return { os, arch };
}
