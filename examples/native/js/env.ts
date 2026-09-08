/**
 * Test-environment helpers for the example e2e suites: release
 * artifact path resolution only. The loader itself lives in
 * `packages/bffi-loader`; the hand-written verification-export
 * harness lives in `load.ts`.
 */

const LIB_EXT =
  process.platform === "win32"
    ? "dll"
    : process.platform === "darwin"
      ? "dylib"
      : "so";

const LIB_PREFIX = process.platform === "win32" ? "" : "lib";

/** Absolute path of the release cdylib artifact. */
export function artifactPath(): string {
  // Both callers sit two levels below the repo root (js/, sequential/):
  // splice(-3) lands there from either. Forward slashes everywhere:
  // Bun's fs/ffi accept them on every platform.
  const parts = import.meta.dir.replaceAll("\\", "/").split("/");
  parts.splice(-3);
  return `${parts.join("/")}/target/release/${LIB_PREFIX}bffi_example_native.${LIB_EXT}`;
}

/** Whether the release artifact has been built (`bun run build`). */
export async function hasArtifact(): Promise<boolean> {
  return await Bun.file(artifactPath()).exists();
}
