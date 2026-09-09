/** `bffi doctor [--config <p>] [--root <d>]`: environment + project
 * diagnostics - bun runtime, cargo executable, config, loader JSON,
 * generated file, artifact. No building. Exit 0 when everything
 * passes, 2 otherwise. */
import { findProjectRoot } from "@z2net/bffi";
import { flagString, parseArgs } from "../args.ts";
import { runDoctorChecks, type CheckResult } from "../checks.ts";
import { EXIT, writeOut } from "../output.ts";

export const usage = `bffi doctor [--config <p>] [--root <d>]`;

export async function doctor(argv: string[]): Promise<number> {
  const args = parseArgs(argv);
  const config = flagString(args, "config");
  const rootFlag = flagString(args, "root");

  const resolved =
    config !== undefined
      ? config.slice(0, config.replaceAll("\\", "/").lastIndexOf("/.bffi/"))
      : (await findProjectRoot(rootFlag)) ?? "";
  if (resolved.length === 0) {
    writeOut("FAIL  config (.bffi/bffi.json) - not found (walked up from the working directory)");
    return EXIT.fail;
  }

  const run = await runDoctorChecks(resolved);
  const results: (CheckResult & { cargo?: boolean })[] = run.results;
  // The cargo check needs the config; when the config itself failed
  // there is nothing to probe, which runDoctorChecks already encodes
  // by omitting the entry.
  print(results);
  return run.allOk ? EXIT.ok : EXIT.fail;
}

/** Prints one line per check: `ok   name - detail` / `FAIL name - detail`. */
function print(results: (CheckResult & { cargo?: boolean })[]): void {
  for (const r of results) {
    const mark = r.ok ? "ok  " : "FAIL";
    const detail = r.detail.length > 0 ? ` - ${r.detail}` : "";
    writeOut(`${mark}  ${r.name}${detail}`);
  }
}
