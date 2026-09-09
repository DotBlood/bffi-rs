/**
 * The `bffi` CLI - a thin wrapper over `@z2net/bffi` until the CLI
 * moves to its own project. Product form:
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
import {
  bunVersionProblem,
  DEFAULT_RUNTIME,
  renderModule,
  SchemaValidationError,
} from "@z2net/bffi";

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
  let command: string | undefined;
  let input: string | undefined;
  let out: string | undefined;
  let runtime: string | undefined;
  let help = false;

  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === "--help" || arg === "-h") {
      help = true;
    } else if (arg === "--out" || arg === "-o") {
      out = argv[i + 1];
      i++;
    } else if (arg === "--runtime") {
      runtime = argv[i + 1];
      i++;
    } else if (arg !== undefined && !arg.startsWith("-")) {
      if (command === undefined) {
        command = arg;
      } else if (input === undefined) {
        input = arg;
      } else {
        throw new UsageError(USAGE, false);
      }
    } else {
      throw new UsageError(USAGE, false);
    }
  }

  if (help) {
    throw new UsageError(USAGE, true);
  }
  if (command !== "codegen" || input === undefined || out === undefined) {
    throw new UsageError(USAGE, false);
  }
  return { input, out, runtime: runtime ?? DEFAULT_RUNTIME };
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
