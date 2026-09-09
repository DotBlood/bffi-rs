/**
 * Tests for platform-binary resolution (pattern A): the napi-rs style
 * triple mapping, the artifact-convention file name, and the
 * `<base>-<triple>` -> binary-path resolution (with an injected
 * resolver - no real node_modules involved).
 *
 * Run with `bun test packages/bffi`.
 */
import { describe, expect, test } from "bun:test";
import { dirname, join } from "node:path";

import {
  platformTriple,
  resolvePlatformBinary,
} from "../src/index.ts";

describe("platformTriple", () => {
  test("maps the shipped platforms onto napi-rs triples", () => {
    expect(platformTriple("win32", "x64")).toBe("win32-x64-msvc");
    expect(platformTriple("linux", "x64")).toBe("linux-x64-gnu");
    expect(platformTriple("linux", "arm64")).toBe("linux-arm64-gnu");
    expect(platformTriple("darwin", "arm64")).toBe("darwin-aarch64");
    expect(platformTriple("darwin", "x64")).toBe("darwin-x64");
  });

  test("rejects platforms bffi does not ship", () => {
    expect(() => platformTriple("freebsd", "x64")).toThrow(/unsupported platform/);
    expect(() => platformTriple("win32", "arm64")).toThrow(/unsupported platform/);
  });
});

describe("resolvePlatformBinary", () => {
  // Expected values are built with the SAME join the implementation
  // uses, so the assertions are platform-separator agnostic.
  test("resolves <base>-<triple> and appends the artifact convention", () => {
    let seenSpec = "";
    let seenFrom = "";
    const entry = "C:/proj/node_modules/@z2net/mylib-win32-x64-msvc/index.js";
    const path = resolvePlatformBinary("@z2net/mylib", {
      triple: "win32-x64-msvc",
      binary: "bffi_mylib",
      from: "C:/proj",
      resolveSync: (specifier, from) => {
        seenSpec = specifier;
        seenFrom = from;
        return entry;
      },
    });
    expect(seenSpec).toBe("@z2net/mylib-win32-x64-msvc");
    expect(seenFrom).toBe("C:/proj");
    expect(path).toBe(join(dirname(entry), "bffi_mylib.dll"));
  });

  test("unix triples carry the lib prefix", () => {
    const entry = "/pkg/node_modules/@z2net/mylib-linux-x64-gnu/index.js";
    const path = resolvePlatformBinary("@z2net/mylib", {
      triple: "linux-x64-gnu",
      binary: "bffi_mylib",
      resolveSync: () => entry,
    });
    expect(path).toBe(join(dirname(entry), "libbffi_mylib.so"));
  });

  test("darwin triples use the dylib extension with the lib prefix", () => {
    const entry = "/pkg/node_modules/@z2net/mylib-darwin-aarch64/index.js";
    const path = resolvePlatformBinary("@z2net/mylib", {
      triple: "darwin-aarch64",
      binary: "bffi_mylib",
      resolveSync: () => entry,
    });
    expect(path).toBe(join(dirname(entry), "libbffi_mylib.dylib"));
  });

  test("a missing platform package produces an actionable error", () => {
    expect(() =>
      resolvePlatformBinary("@z2net/mylib", {
        triple: "linux-x64-gnu",
        binary: "bffi_mylib",
        resolveSync: () => {
          throw new Error("MODULE_NOT_FOUND");
        },
      }),
    ).toThrow(/@z2net\/mylib-linux-x64-gnu is not installed/);
  });
  test("a missing binary option is rejected up front", () => {
    // The binary check runs BEFORE any resolution, so no default
    // resolver is consulted here. The option is required in the TS
    // types; the runtime guard covers untyped (JS) consumers.
    const untyped = resolvePlatformBinary as unknown as (
      base: string,
      options: Record<string, unknown>,
    ) => string;
    expect(() =>
      untyped("@z2net/mylib", { triple: "win32-x64-msvc" }),
    ).toThrow(/options\.binary is required/);
  });
});
