/**
 * Process-level exit-code contract of the CLI (`bun bin/index.ts`):
 * 0 on success, 1 on usage errors, 2 on input failures.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const ENTRY = fileURLToPath(new URL("./index.ts", import.meta.url));

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
  const dir = mkdtempSync(join(tmpdir(), "bffi-cli-"));

  test("codegen succeeds and writes the module (exit 0)", () => {
    const input = join(dir, "schema.json");
    const out = join(dir, "out.gen.ts");
    writeFileSync(input, GOOD_SCHEMA);
    const { code } = run(["codegen", input, "-o", out]);
    expect(code).toBe(0);
    expect(existsSync(out)).toBeTrue();
    expect(readFileSync(out, "utf8")).toContain("Source module: probe");
  });

  test("a missing positional is a usage error (exit 1)", () => {
    const { code } = run(["codegen"]);
    expect(code).toBe(1);
  });

  test("--help exits 0", () => {
    const { code } = run(["--help"]);
    expect(code).toBe(0);
  });

  test("unparsable JSON is an input failure (exit 2)", () => {
    const input = join(dir, "bad.json");
    writeFileSync(input, "{ not json");
    const { code, stderr } = run(["codegen", input, "-o", join(dir, "out.gen.ts")]);
    expect(code).toBe(2);
    expect(stderr).toContain("cannot read or parse JSON");
  });

  test("schema validation failures exit 2 with diagnostics", () => {
    const input = join(dir, "schema.json");
    writeFileSync(input, JSON.stringify({ bffi: 2, module: "x", functions: [], classes: [] }));
    const { code, stderr } = run(["codegen", input, "-o", join(dir, "out.gen.ts")]);
    expect(code).toBe(2);
    expect(stderr).toContain("$.bffi");
  });

  afterAll(() => {
    rmSync(dir, { recursive: true, force: true });
  });
});
