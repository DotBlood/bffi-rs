/**
 * `.bffi` working-directory constants plus the path/file-URL
 * helpers.
 *
 * Bun-only: the file URL comes from the runtime built-in
 * `Bun.pathToFileURL`, and path joining is the pure-string
 * [`joinOut`] - no `node:` module imports anywhere in the package.
 */

/** The `.bffi` working directory name (relative). */
export const BFFI_DIR = ".bffi";

/** The config file name inside `.bffi`. */
export const CONFIG_FILE = "bffi.json";

/** A `file://` URL for an absolute path (dynamic `import()` on
 * Windows requires the URL form). */
export function fileUrl(path: string): string {
  return Bun.pathToFileURL(path).href;
}

/** Joins `root` with every part of `parts` in order. Each part is
 * normalized to forward slashes; an ABSOLUTE part (`/x` or `C:/x`)
 * wins over everything joined before it (resolve-style), and empty
 * or `.` parts are skipped. */
export function joinOut(root: string, ...parts: string[]): string {
  let joined = root;
  for (const part of parts) {
    const normalized = part.replaceAll("\\", "/");
    if (normalized.length === 0 || normalized === "." || normalized === "./") {
      continue;
    }
    if (normalized.startsWith("/") || /^[A-Za-z]:\//.test(normalized)) {
      joined = normalized;
      continue;
    }
    joined = `${joined.replace(/\/+$/, "")}/${normalized.replace(/^\.\/+/, "").replace(/^\/+/, "")}`;
  }
  return joined;
}
