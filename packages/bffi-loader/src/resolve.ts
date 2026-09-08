/**
 * Platform-binary resolution for pattern-A distribution (napi-rs
 * style): the main package declares every platform package in
 * `optionalDependencies`; npm/bun installs ONLY the one matching the
 * running platform (`os`/`cpu`/`libc` fields), and this module turns
 * that package into the dlopen path.
 *
 * Triple naming follows the napi-rs convention
 * (`<base>-win32-x64-msvc`, `<base>-linux-x64-gnu`,
 * `<base>-darwin-aarch64`, ...); a platform package's `index.js`
 * exports `{ path: "<absolute binary path>" }`, so the binary's file
 * name is the platform package's own concern.
 */
import { createRequire } from "node:module";

/** The napi-rs style triple of the RUNNING platform. Throws for
 * platforms bffi does not ship. */
export function platformTriple(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): string {
  const key = `${platform}-${arch}`;
  switch (key) {
    case "win32-x64":
      return "win32-x64-msvc";
    case "linux-x64":
      return "linux-x64-gnu";
    case "linux-arm64":
      return "linux-arm64-gnu";
    case "darwin-arm64":
      return "darwin-aarch64";
    case "darwin-x64":
      return "darwin-x64";
    default:
      throw new Error(
        `unsupported platform for bffi native packages: ${key} ` +
          `(shipped: win32-x64, linux-x64, linux-arm64, darwin-x64, darwin-arm64)`,
      );
  }
}

/** Options of [`resolvePlatformBinary`]; every field defaults to the
 * running environment. `resolvePackage` is injectable for tests. */
export interface ResolveOptions {
  /** Override the detected platform triple. */
  triple?: string;
  /** Module resolver; defaults to `createRequire(import.meta.url)`. */
  resolvePackage?: (packageName: string) => unknown;
}

/**
 * Resolves the absolute path of the native binary behind the
 * platform package `<base>-<triple>` of `base` (e.g.
 * `@z2net/mylib` -> `@z2net/mylib-win32-x64-msvc`).
 *
 * Throws a clear error when the platform package is not installed
 * (optional dependencies can be skipped by package managers) or does
 * not export a `path`.
 */
export function resolvePlatformBinary(
  base: string,
  options: ResolveOptions = {},
): string {
  const triple = options.triple ?? platformTriple();
  const packageName = `${base}-${triple}`;
  let mod: unknown;
  try {
    if (options.resolvePackage !== undefined) {
      mod = options.resolvePackage(packageName);
    } else {
      const require = createRequire(import.meta.url);
      mod = require(packageName);
    }
  } catch (error) {
    throw new Error(
      `native package ${packageName} is not installed or failed to load ` +
        `(${String(error)}). Install it explicitly or pass an absolute ` +
        `library path instead.`,
    );
  }
  const path = (mod as { path?: unknown } | null | undefined)?.path;
  if (typeof path !== "string" || path.length === 0) {
    throw new Error(
      `native package ${packageName} must export { path: string } ` +
        `(the absolute path of the binary)`,
    );
  }
  return path;
}
