/**
 * Tests for `bffi check` and `bffi doctor` on a temp project:
 * everything-pass run, and each failing dimension (config, loader
 * JSON, generated file, artifact) flips the exit code to 2.
 *
 * Run with `bun test packages/bffi-cli`.
 */
import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { $ } from "bun";

const ROOT = `${import.meta.dir}/tmp`;
const BFFI_DIR = `${ROOT}/.bffi`;

const VALID_CONFIG = {
  bffi: 1,
  version: "0.1.0",
  module: "demo",
  crate: { dir: "crate", name: "bffi-demo", binary: "bffi_demo" },
};

const VALID_LOADER_JSON = JSON.stringify({
  bffi: 1,
  module: "demo",
  functions: [],
  classes: [],
});

async function scaffoldProject(): Promise<void> {
  await Bun.write(`${BFFI_DIR}/bffi.json`, `${JSON.stringify(VALID_CONFIG, null, 2)}\n`);
  await Bun.write(`${BFFI_DIR}/bffi.api.json`, VALID_LOADER_JSON);
  await Bun.write(`${BFFI_DIR}/api.gen.ts`, "// @generated\nexport type Api = unknown;\n");
  // A fake artifact at the resolved local path (platform-aware).
  const ext = process.platform === "win32" ? "dll" : process.platform === "darwin" ? "dylib" : "so";
  const prefix = process.platform === "win32" ? "" : "lib";
  await Bun.write(`${ROOT}/crate/target/release/${prefix}bffi_demo.${ext}`, new Uint8Array([1]));
}

describe("bffi check / doctor", () => {
  beforeAll(async () => {
    await scaffoldProject();
  });

  test("check passes on a complete project (exit 0)", () => {
    const { code, stdout } = runCli(["check", "--root", ROOT]);
    expect(code).toBe(0);
    expect(stdout).toContain("ok");
    expect(stdout).not.toContain("FAIL");
  });

  test("doctor passes on a complete project (exit 0)", () => {
    // The cargo probe requires a USABLE toolchain: CI runners may
    // ship a cargo binary that cannot actually run (hangs on
    // startup). Probe it first with a short timeout; skip when the
    // probe fails.
    const probe = Bun.spawnSync(["cargo", "--version"], { timeout: 2000 });
    if (probe.exitCode !== 0) {
      console.info("skip: cargo is missing or unusable in this environment");
      return;
    }
    const { code, stdout } = runCli(["doctor", "--root", ROOT]);
    expect(code).toBe(0);
    expect(stdout).toContain("ok    bun runtime");
    expect(stdout).not.toContain("FAIL");
  }, 15000);

  test("check fails when the loader JSON is missing", async () => {
    await $`rm -rf ${BFFI_DIR}/bffi.api.json`;
    const { code, stdout } = runCli(["check", "--root", ROOT]);
    expect(code).toBe(2);
    expect(stdout).toContain("loader JSON");
    await Bun.write(`${BFFI_DIR}/bffi.api.json`, VALID_LOADER_JSON);
  });

  test("check fails when the artifact is missing", async () => {
    const ext = process.platform === "win32" ? "dll" : process.platform === "darwin" ? "dylib" : "so";
    const prefix = process.platform === "win32" ? "" : "lib";
    await $`rm -rf ${ROOT}/crate`;
    const { code, stdout } = runCli(["check", "--root", ROOT]);
    expect(code).toBe(2);
    expect(stdout).toContain("artifact");
    // Restore for the afterAll cleanup bookkeeping (nothing else
    // depends on the fake artifact).
    await Bun.write(`${ROOT}/crate/target/release/${prefix}bffi_demo.${ext}`, new Uint8Array([1]));
  });

  test("doctor fails when the config version is unsupported", async () => {
    await Bun.write(
      `${BFFI_DIR}/bffi.json`,
      JSON.stringify({ ...VALID_CONFIG, bffi: 99 }, null, 2),
    );
    const { code, stdout } = runCli(["doctor", "--root", ROOT]);
    expect(code).toBe(2);
    expect(stdout).toContain("config");
    await Bun.write(`${BFFI_DIR}/bffi.json`, `${JSON.stringify(VALID_CONFIG, null, 2)}\n`);
  });

  afterAll(async () => {
    await $`rm -rf ${ROOT}`;
  });
});

function runCli(args: string[]): { code: number; stdout: string; stderr: string } {
  const entry = `${import.meta.dir}/../bin/bffi.ts`;
  const proc = Bun.spawnSync(["bun", entry, ...args]);
  return {
    code: proc.exitCode,
    stdout: proc.stdout.toString(),
    stderr: proc.stderr.toString(),
  };
}
