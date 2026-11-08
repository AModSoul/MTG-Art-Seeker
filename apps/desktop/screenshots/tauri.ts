// Stands in for the `@tauri-apps/*` modules the app imports while the screenshots are taken.
// `src/api.ts`, which calls Tauri itself, is swapped for `api.ts`.

/** Saved art is read straight off the disk, through the dev server's `/@fs/` route, which
 * takes a path with forward slashes: `/home/…`, or `C:/Users/…` on Windows, where the
 * recording's paths are `\\?\C:\Users\…`. */
export const convertFileSrc = (path: string) => {
  const forward = path.replace(/^\\\\\?\\/, "").replaceAll("\\", "/");
  return `/@fs${forward.startsWith("/") ? "" : "/"}${encodeURI(forward)}`;
};

// plugin-dialog: nothing is picked. plugin-updater: no update. plugin-process: nothing to do.
export const open = async () => null;
export const check = async () => null;
export const relaunch = async () => {};
