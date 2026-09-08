/**
 * Tests for platform-binary resolution (pattern A): the napi-rs style
 * triple mapping and the `<base>-<triple>` -> binary-path resolution
 * (with an injected resolver - no real node_modules involved).
 *
 * Run with `bun test packages/bffi-loader`.
 */
import { describe, expect, test } from "bun:test";

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
  const win32Resolver = (name: string): unknown => {
    expect(name).toBe("@z2net/mylib-win32-x64-msvc");
    return { path: "C:/pkg/mylib.dll" };
  };

  test("resolves <base>-<triple> and returns its exported path", () => {
    expect(
      resolvePlatformBinary("@z2net/mylib", {
        triple: "win32-x64-msvc",
        resolvePackage: win32Resolver,
      }),
    ).toBe("C:/pkg/mylib.dll");
  });

  test("appends the default (running) triple to the base name", () => {
    // The running platform triple is substituted; the stub only
    // accepts that exact package name, which asserts the formation.
    const triple = platformTriple();
    let seen = "";
    resolvePlatformBinary("@z2net/mylib", {
      resolvePackage: (name: string) => {
        seen = name;
        return { path: "/pkg/libmy.so" };
      },
    });
    expect(seen).toBe(`@z2net/mylib-${triple}`);
  });

  test("a missing platform package produces an actionable error", () => {
    expect(() =>
      resolvePlatformBinary("@z2net/mylib", {
        triple: "linux-x64-gnu",
        resolvePackage: () => {
          throw new Error("MODULE_NOT_FOUND");
        },
      }),
    ).toThrow(/@z2net\/mylib-linux-x64-gnu is not installed/);
  });

  test("a platform package without a path export is rejected", () => {
    expect(() =>
      resolvePlatformBinary("@z2net/mylib", {
        triple: "darwin-aarch64",
        resolvePackage: () => ({}),
      }),
    ).toThrow(/must export \{ path: string \}/);
  });
});
