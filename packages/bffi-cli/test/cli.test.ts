/**
 * Process-level tests of the CLI: exit codes via spawning
 * `bin/bffi.ts` (0 ok / 1 usage / 2 input).
 *
 * Run with `bun test packages/bffi-cli`.
 */
import { afterAll, describe, expect, test } from "bun:test";

const ENTRY = `${import.meta.dir}/../bin/bffi.ts`;

function run(args: string[]): { code: number; stderr: string } {
  const proc = Bun.spawnSync(["bun", ENTRY, ...args]);
  return {
    code: proc.exitCode,
    stderr: proc.stderr.toString(),
  };
}

describe("bffi CLI exit codes", () => {
  const dir = `${import.meta.dir}/tmp`;

  test("codegen succeeds and writes the module (exit 0)", async () => {
    const input = `${dir}/schema.json`;
    const out = `${dir}/out.gen.ts`;
    await Bun.write(
      input,
      JSON.stringify({ bffi: 1, module: "probe", functions: [], classes: [] }),
    );
    const { code } = run(["codegen", input, "-o", out]);
    expect(code).toBe(0);
    expect(await Bun.file(out).exists()).toBeTrue();
  });

  test("a missing positional is a usage error (exit 1)", () => {
    const { code } = run(["codegen"]);
    expect(code).toBe(1);
  });

  test("an unknown command is a usage error (exit 1)", () => {
    const { code } = run(["frobnicate"]);
    expect(code).toBe(1);
  });

  test("--help exits 0", () => {
    const { code } = run(["--help"]);
    expect(code).toBe(0);
  });

  test("unparsable JSON is an input failure (exit 2)", async () => {
    const input = `${dir}/bad.json`;
    await Bun.write(input, "{ not json");
    const { code, stderr } = run(["codegen", input, "-o", `${dir}/out.gen.ts`]);
    expect(code).toBe(2);
    expect(stderr).toContain("cannot read or parse JSON");
  });

  test("schema validation failures exit 2 with diagnostics", async () => {
    const input = `${dir}/schema.json`;
    await Bun.write(input, JSON.stringify({ bffi: 2, module: "x", functions: [], classes: [] }));
    const { code, stderr } = run(["codegen", input, "-o", `${dir}/out.gen.ts`]);
    expect(code).toBe(2);
    expect(stderr).toContain("$.bffi");
  });

  afterAll(async () => {
    const { $ } = await import("bun");
    await $`rm -rf ${dir}`;
  });
});
