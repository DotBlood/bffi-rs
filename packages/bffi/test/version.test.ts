/**
 * Tests for the runtime Bun version gate: the numeric comparator
 * (`1.10.0` must be NEWER than `1.4.0` - the regression a string
 * comparison would fail), prerelease suffixes, the no-runtime case,
 * and the assert/problem-message shapes.
 *
 * Run with `bun test packages/bffi`.
 */
import { describe, expect, test } from "bun:test";

import {
  assertBunVersion,
  bunVersionProblem,
  bunVersionSatisfies,
  MIN_BUN_VERSION,
} from "../src/index.ts";

describe("bunVersionSatisfies", () => {
  test("the minimum itself satisfies", () => {
    expect(bunVersionSatisfies("1.4.0")).toBeTrue();
    expect(bunVersionSatisfies("1.4")).toBeTrue();
  });

  test("older versions are rejected", () => {
    expect(bunVersionSatisfies("1.3.9")).toBeFalse();
    expect(bunVersionSatisfies("1.3")).toBeFalse();
    expect(bunVersionSatisfies("1.0.0")).toBeFalse();
    expect(bunVersionSatisfies("0.9.1")).toBeFalse();
  });

  test("numeric comparison: 1.10.0 is NEWER than 1.4.0", () => {
    expect(bunVersionSatisfies("1.10.0")).toBeTrue();
    expect(bunVersionSatisfies("1.9.9")).toBeTrue();
    expect(bunVersionSatisfies("2.0.0")).toBeTrue();
  });

  test("prerelease suffixes compare by the leading triple", () => {
    expect(bunVersionSatisfies("1.4.0-canary.12")).toBeTrue();
    expect(bunVersionSatisfies("1.4.1-beta.1")).toBeTrue();
    expect(bunVersionSatisfies("1.3.0-canary.1")).toBeFalse();
  });

  test("undefined and garbage are rejected", () => {
    expect(bunVersionSatisfies(undefined)).toBeFalse();
    expect(bunVersionSatisfies("")).toBeFalse();
    expect(bunVersionSatisfies("unknown")).toBeFalse();
  });

  test("a custom minimum is honored", () => {
    expect(bunVersionSatisfies("1.4.0", "1.5.0")).toBeFalse();
    expect(bunVersionSatisfies("1.5.0", "1.5.0")).toBeTrue();
  });

  test("MIN_BUN_VERSION is the AGENTS.md floor", () => {
    expect(MIN_BUN_VERSION).toBe("1.4.0");
  });
});

describe("bunVersionProblem / assertBunVersion", () => {
  test("a satisfying version yields no problem and no throw", () => {
    expect(bunVersionProblem("1.4.0")).toBeUndefined();
    expect(() => assertBunVersion("1.4.0")).not.toThrow();
  });

  test("an old version produces an upgrade message", () => {
    expect(bunVersionProblem("1.3.2")).toBe(
      "requires Bun >= 1.4.0; found 1.3.2. Upgrade Bun: https://bun.sh",
    );
    expect(() => assertBunVersion("1.3.2")).toThrow(
      /@z2net\/bffi requires Bun >= 1\.4\.0; found 1\.3\.2/,
    );
  });

  test("a missing runtime produces the no-Bun message", () => {
    expect(bunVersionProblem(undefined)).toBe(
      "requires Bun >= 1.4.0; no Bun runtime detected",
    );
  });
});
