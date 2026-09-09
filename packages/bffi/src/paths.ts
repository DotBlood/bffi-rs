/**
 * `.bffi` working-directory constants plus the file-URL helper.
 *
 * Path joining/manipulation uses the runtime built-in `node:path`
 * module directly (Bun implements it natively - no external
 * dependency); this module only carries the bffi-specific constants
 * and the one wrapper dynamic `import()` needs.
 */
import { pathToFileURL } from "node:url";

/** The `.bffi` working directory name (relative). */
export const BFFI_DIR = ".bffi";

/** The config file name inside `.bffi`. */
export const CONFIG_FILE = "bffi.json";

/** A `file://` URL for an absolute path (dynamic `import()` on
 * Windows requires the URL form). */
export function fileUrl(path: string): string {
  return pathToFileURL(path).href;
}
