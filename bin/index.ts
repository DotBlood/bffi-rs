/**
 * The `bffi` CLI. Product form (v1):
 *
 *   bun bffi codegen <input.json> -o <out.ts> [--runtime <module>]
 *
 * Exit codes: 0 = ok, 1 = usage error, 2 = input read/parse/validation
 * failure (diagnostics on stderr, one `path: message` line each).
 *
 * The renderer is deterministic: the same input file yields a
 * byte-identical module, which makes the generated file safe to
 * commit and diff.
 */
import { parseArgs } from "node:util";
import { renderModule, DEFAULT_RUNTIME } from "./codegen.ts";
import { SchemaValidationError } from "./schema.ts";
import { bunVersionProblem } from "../packages/bffi-loader/src/version.ts";

const USAGE = `bffi - codegen for bffi-rs native modules

Usage:
  bun bffi codegen <input.json> -o <out.ts> [--runtime <module>]

Arguments:
  <input.json>        the loader schema (bffi_build::loader_json output)
  -o, --out <out.ts>  the generated TypeScript module path
  --runtime <module>  the runtime import specifier (default: ${DEFAULT_RUNTIME})
  -h, --help          show this help
`;

interface Options {
  input: string;
  out: string;
  runtime: string;
}

class UsageError extends Error {
  /** Whether the usage text was REQUESTED (exit 0) or is an error (1). */
  readonly requested: boolean;

  constructor(message: string, requested: boolean) {
    super(message);
    this.name = "UsageError";
    this.requested = requested;
  }
}

/** Parses argv; throws a [`UsageError`] on anything unexpected. */
function parseOptions(argv: string[]): Options {
  const { values, positionals } = parseArgs({
    args: argv,
    options: {
      out: { type: "string", short: "o" },
      runtime: { type: "string" },
      help: { type: "boolean", short: "h" },
    },
    allowPositionals: true,
  });
  if (values.help) {
    throw new UsageError(USAGE, true);
  }
  const [command, input] = positionals;
  if (command !== "codegen" || input === undefined || positionals.length !== 2) {
    throw new UsageError(USAGE, false);
  }
  if (values.out === undefined) {
    throw new UsageError(USAGE, false);
  }
  return {
    input,
    out: values.out,
    runtime: values.runtime ?? DEFAULT_RUNTIME,
  };
}

/** The process entry point; resolves to the exit code. */
export async function main(argv: string[]): Promise<number> {
  // Environment gate first (exit 2: neither a usage nor an input
  // error - the runtime itself is too old).
  const versionProblem = bunVersionProblem(Bun.version);
  if (versionProblem !== undefined) {
    process.stderr.write(`bffi ${versionProblem}\n`);
    return 2;
  }

  let options: Options;
  try {
    options = parseOptions(argv);
  } catch (error) {
    if (error instanceof UsageError) {
      process.stderr.write(error.message);
      return error.requested ? 0 : 1;
    }
    throw error;
  }

  let raw: unknown;
  try {
    const text = await Bun.file(options.input).text();
    raw = JSON.parse(text);
  } catch (error) {
    process.stderr.write(
      `${options.input}: cannot read or parse JSON: ${String(error)}\n`,
    );
    return 2;
  }

  let rendered: string;
  try {
    rendered = renderModule(raw, options.runtime);
  } catch (error) {
    if (error instanceof SchemaValidationError) {
      for (const issue of error.issues) {
        process.stderr.write(`${options.input} ${issue.path}: ${issue.message}\n`);
      }
      return 2;
    }
    throw error;
  }

  await Bun.write(options.out, rendered);
  return 0;
}

// The direct-run entry point (tests import `main` instead).
if (import.meta.main) {
  void main(process.argv.slice(2)).then((code) => {
    process.exitCode = code;
  });
}
