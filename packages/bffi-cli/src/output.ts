/** Exit codes of the CLI. */
export const EXIT = {
  ok: 0,
  usage: 1,
  fail: 2,
} as const;

/** Writes a line to stderr. */
export function writeErr(line: string): void {
  process.stderr.write(`${line}\n`);
}

/** Writes a line to stdout. */
export function writeOut(line: string): void {
  process.stdout.write(`${line}\n`);
}
