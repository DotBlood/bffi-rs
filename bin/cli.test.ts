/**
 * Process-level exit-code contract of the CLI (`bun bin/index.ts`):
 * 0 on success, 1 on usage errors, 2 on input failures.
 *
 * Bun-only: file scaffolding via `Bun.write` (creates parent dirs),
 * cleanup via the cross-platform Bun Shell.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { $ } from "bun";

const ENTRY = `${import.meta.dir}/index.ts`;

const GOOD_SCHEMA = JSON.stringify({
  bffi: 1,
  module: "probe",
  functions: [],
  classes: [],
});

function run(args: string[]): { code: number; stderr: string } {
  const proc = Bun.spawnSync(["bun", ENTRY, ...args]);
  return {
    code: proc.exitCode,
    stderr: proc.stderr.toString(),
  };
}

describe("bffi CLI exit codes", () => {
  const dir = `${import.meta.dir}/tmp`;

  afterAll(async () => {
    await $`rm -rf ${dir}`;
  });

  test("codegen succeeds and writes the module (exit 0)", async () => {
    const input = `${dir}/schema.json`;
    const out = `${dir}/out.gen.ts`;
    await Bun.write(input, GOOD_SCHEMA);
    const { code } = run(["codegen", input, "-o", out]);
    expect(code).toBe(0);
    expect(await Bun.file(out).exists()).toBeTrue();
    expect(await Bun.file(out).text()).toContain("Source module: probe");
  });

  test("a missing positional is a usage error (exit 1)", () => {
    const { code } = run(["codegen"]);
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
});
