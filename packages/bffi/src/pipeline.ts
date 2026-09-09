/**
 * The pipeline orchestrator: ONE call from the consumer.
 *
 * ```ts
 * import { bffi } from "@z2net/bffi";
 * import type { Api } from "../.bffi/api.gen.ts";
 *
 * const api: Api = await bffi(); // build -> json -> gen -> resolve -> dlopen
 * ```
 *
 * `bffiBuild()` / `bffiGenerate()` / `bffiLoad()` expose individual
 * pipeline steps for CI and scripts.
 */
import {
  applyDebug,
  debugLog,
} from "./debug.ts";
import {
  findProjectRoot,
  loadConfigFile,
  type BffiConfig,
} from "./config.ts";
import { buildCrate } from "./build.ts";
import { generateApiGen } from "./generate.ts";
import { join as joinPath } from "node:path";
import { validateModule } from "./schema.ts";
import { createApi } from "./api.ts";

/** Options of [`bffi`] (the full pipeline). */
export interface BffiOptions {
  /** Explicit path of `.bffi/bffi.json`; default: discovered from
   * the working directory upward. */
  config?: string;
  /** Skip the cargo build step (the artifact is already fresh). */
  skipBuild?: boolean;
  /** Override the config's debug flag. */
  debug?: boolean;
}

/** Locates the project root: an explicit config file path wins,
 * otherwise `.bffi/bffi.json` is discovered from the working
 * directory upward. */
async function locateRoot(options: BffiOptions): Promise<string> {
  if (options.config !== undefined) {
    const normalized = options.config.replaceAll("\\", "/");
    const cut = normalized.lastIndexOf("/.bffi/");
    if (cut > 0) {
      return normalized.slice(0, cut);
    }
    return normalized.slice(0, normalized.lastIndexOf("/"));
  }
  const found = await findProjectRoot();
  if (found === undefined) {
    throw new Error(
      "bffi config not found: no ancestor directory contains .bffi/bffi.json. " +
        "Create .bffi/bffi.json (see defineConfig) or pass { config } explicitly.",
    );
  }
  return found;
}

/** Reads and validates the loader JSON working file
 * (`.bffi/<files[0]>`). */
async function readLoaderJson(
  root: string,
  config: BffiConfig,
): Promise<unknown> {
  const inputName = config.files[0] ?? "bffi.api.json";
  const path = joinPath(root, ".bffi", inputName);
  const file = Bun.file(path);
  if (!(await file.exists())) {
    throw new Error(
      `loader JSON not found: ${path}. Run the crate build first ` +
        "(its build step emits the file) or pass libraryPath.",
    );
  }
  return validateModule(JSON.parse(await file.text()));
}

/**
 * The FULL pipeline: cargo build -> loader JSON -> api.gen generation
 * -> binary resolution -> dlopen. Returns the typed API (annotate the
 * call site with the generated `Api` type for exact signatures):
 *
 * ```ts
 * import type { Api } from "../.bffi/api.gen.ts";
 * const api: Api = await bffi();
 * ```
 */
export async function bffi<T = unknown>(options: BffiOptions = {}): Promise<T> {
  const root = await locateRoot(options);
  debugLog("project root:", root);

  const config = await loadConfigFile(root);
  applyDebug(options.debug ?? config.debug);

  await buildCrate(config, { root, skip: options.skipBuild });

  await bffiGenerate({ root, config });

  const libraryPath =
    config.libraryPath ??
    localArtifactPath(config, root);

  debugLog("dlopen:", libraryPath);
  // validateModule (in readLoaderJson) has checked the shape; the
  // generic call site is typed through the consumer's `T`.
  const rawModule = await readLoaderJson(root, config);
  return createApi(rawModule as never, libraryPath, config.features ?? {}) as T;
}

/** The LOCAL artifact of the configured crate:
 * `<crate.dir>/<crate.targetDir|target>/release/[lib]<binary>.<ext>`
 * (the os config field overrides the running platform). */
function localArtifactPath(config: BffiConfig, root: string): string {
  const os = config.os === "auto" || config.os === undefined ? process.platform : config.os;
  const { ext, prefix } =
    os === "win32"
      ? { ext: "dll", prefix: "" }
      : os === "darwin"
        ? { ext: "dylib", prefix: "lib" }
        : { ext: "so", prefix: "lib" };
  const targetDir = config.crate.targetDir ?? joinPath(config.crate.dir, "target");
  return joinPath(root, targetDir, "release", `${prefix}${config.crate.binary}.${ext}`);
}

/** Options of [`bffiGenerate`]. */
export interface GenerateOptions {
  /** Project root (default: discovered). */
  root?: string;
  /** Config override (default: `.bffi/bffi.json` under root). */
  config?: BffiConfig;
}

/** Only the generation step: loader JSON -> `.bffi/api.gen.ts`.
 * Returns the absolute path of the generated file. */
export async function bffiGenerate(options: GenerateOptions = {}): Promise<string> {
  const root = options.root ?? (await locateRoot({}));
  const config = options.config ?? (await loadConfigFile(root));
  return generateApiGen(config, root);
}

/** Only the cargo build step (uses `rust` from the config). */
export async function bffiBuild(options: {
  root?: string;
  cargo?: string;
  skip?: boolean;
} = {}): Promise<void> {
  const root = options.root ?? (await locateRoot({}));
  const config = await loadConfigFile(root);
  applyDebug(config.debug);
  await buildCrate(config, { root, cargo: options.cargo, skip: options.skip });
}
