/**
 * The Rust build step of the pipeline: spawns the configured cargo
 * executable against the configured crate. The crate's own `build.rs`
 * is responsible for writing the loader JSON into `.bffi/`.
 */
import { debugLog } from "./debug.ts";
import type { BffiConfig } from "./config.ts";

/** Options of [`buildCrate`]. */
export interface BuildOptions {
  /** Project root (the directory containing `.bffi`); cargo runs
   * from here so the workspace `target/` is shared. */
  root?: string;
  /** The cargo executable (default: config `rust`, then `"cargo"`). */
  cargo?: string;
  /** Skip the build entirely (the artifact is already fresh). */
  skip?: boolean;
}

/**
 * Runs `cargo build --release -p <crate.name>` from `root`.
 * Debug mode (config) is forwarded to the subprocess as
 * `BFFI_DEBUG=1`; output streams through to the terminal.
 */
export async function buildCrate(config: BffiConfig, options: BuildOptions = {}): Promise<void> {
  if (options.skip) {
    debugLog("build skipped");
    return;
  }
  const cargo = options.cargo ?? config.rust;
  const root = options.root ?? process.cwd();
  const args = ["build", "--release", "-p", config.crate.name];
  debugLog("spawning:", cargo, ...args);
  const env: Record<string, string> = { ...process.env } as Record<string, string>;
  if (config.debug) {
    env.BFFI_DEBUG = "1";
  }
  const proc = Bun.spawn([cargo, ...args], {
    cwd: root,
    env,
    stdout: "inherit",
    stderr: "inherit",
    stdin: "ignore",
  });
  const code = await proc.exited;
  if (code !== 0) {
    throw new Error(`cargo build failed with exit code ${code}`);
  }
}
