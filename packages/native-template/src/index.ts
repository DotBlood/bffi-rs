/**
 * COPY-ME template entry: resolves the platform binary (pattern A)
 * and returns the typed API. Types come from the embedded schema
 * literal in `api.gen.ts`.
 */
import { createApiFromJson, type Api } from "./api.gen.ts";
import { resolvePlatformBinary } from "../../bffi-loader/src/index.ts";

export type { Api };

/**
 * Opens the native module and returns the typed API. Without
 * `libraryPath` the platform package
 * (`@z2net/native-template-<triple>`) installed by
 * `optionalDependencies` provides the binary path.
 */
export function createNative(libraryPath?: string): Api {
  return createApiFromJson(libraryPath ?? resolvePlatformBinary("@z2net/native-template"));
}
