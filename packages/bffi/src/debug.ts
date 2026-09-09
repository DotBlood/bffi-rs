/**
 * Debug mode: the `debug` config flag (NOT an env variable the user
 * has to manage) turns on verbose logging and is forwarded to every
 * spawned subprocess as `BFFI_DEBUG=1`.
 */

let debugEnabled = false;

/** Applies the config's debug flag; returns the effective state. */
export function applyDebug(debug: boolean | undefined): boolean {
  debugEnabled = debug === true;
  if (debugEnabled) {
    process.env.BFFI_DEBUG = "1";
  }
  return debugEnabled;
}

/** Whether debug mode is currently enabled. */
export function isDebug(): boolean {
  return debugEnabled;
}

/** Verbose log (printed only in debug mode). */
export function debugLog(...parts: unknown[]): void {
  if (debugEnabled) {
    console.info("[bffi]", ...parts);
  }
}
