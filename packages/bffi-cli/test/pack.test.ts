/**
 * Tests for `bffi pack`: assembles a platform package (binary +
 * os/cpu/libc package.json + `{path}` shim) from a fake binary.
 *
 * Run with `bun test packages/bffi-cli`.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { join } from "node:path";

import { pack } from "../src/commands/pack.ts";

const ROOT = `${import.meta.dir}/tmp`;
const SRC = `${ROOT}/fake.dll`;

describe("bffi pack", () => {
  test("assembles the platform package with the artifact convention", async () => {
    await Bun.write(SRC, new Uint8Array([1, 2, 3]));
    const code = await pack([
      "--src",
      SRC,
      "--triple",
      "win32-x64-msvc",
      "--name",
      "@z2net/mylib",
      "--out",
      `${ROOT}/platform`,
    ]);
    expect(code).toBe(0);

    const pkgPath = join(
      ROOT,
      "platform",
      "mylib-win32-x64-msvc",
      "package.json",
    );
    const pkg = JSON.parse(await Bun.file(pkgPath).text());
    expect(pkg.name).toBe("@z2net/mylib-win32-x64-msvc");
    expect(pkg.os).toEqual(["win32"]);
    expect(pkg.cpu).toEqual(["x64"]);

    const shim = await Bun.file(
      join(ROOT, "platform", "mylib-win32-x64-msvc", "index.js"),
    ).text();
    expect(shim).toContain('"native.dll"');

    expect(
      await Bun.file(join(ROOT, "platform", "mylib-win32-x64-msvc", "native.dll")).exists(),
    ).toBeTrue();
  });

  test("an unknown triple fails (exit 2)", async () => {
    const code = await pack([
      "--src",
      SRC,
      "--triple",
      "freebsd-x64",
      "--out",
      `${ROOT}/platform`,
    ]);
    expect(code).toBe(2);
  });

  test("a missing --src is a usage error (exit 1)", async () => {
    expect(await pack(["--triple", "win32-x64-msvc"])).toBe(1);
  });

  afterAll(async () => {
    const { $ } = await import("bun");
    await $`rm -rf ${ROOT}`;
  });
});
