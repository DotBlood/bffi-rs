/**
 * The `@z2net/bffi-native` entry: resolves the prebuilt binary of
 * the RUNNING platform (installed as an optional dependency by
 * pattern A) and opens it through the `@z2net/bffi` typed loader.
 *
 * An explicit `libraryPath` overrides the resolution (useful for
 * tests and locally built artifacts).
 */
import { resolvePlatformBinary } from "@z2net/bffi";

import { createApiFromJson, type Api } from "./api.gen.ts";

export type { Api };

/** Opens the prebuilt native library and returns the typed API. */
export function createNative(libraryPath?: string): Api {
  return createApiFromJson(
    libraryPath ??
      resolvePlatformBinary("@z2net/bffi-native", { binary: "bffi_native" }),
  );
}
