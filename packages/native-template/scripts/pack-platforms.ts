/**
 * COPY-ME template script: assembles one platform package (pattern A)
 * from a built cdylib:
 *
 *   bun scripts/pack-platforms.ts --src target/release/native.dll --triple win32-x64-msvc
 *
 * Output: platform/<base>-<triple>/ with the binary (renamed to the
 * convention), a package.json carrying os/cpu/libc, and an index.js
 * shim exporting `{ path }` - the only contract `bffi-loader`'
 * `resolvePlatformBinary` relies on.
 *
 * Edit PLATFORMS/BINARY_NAMES when your crate's artifact names differ.
 */
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** triple -> npm os/cpu/libc fields + the binary file name inside the
 * platform package. Extend the map when you add targets. */
const PLATFORMS: Record<
  string,
  { os: string; cpu: string; libc?: string; file: string }
> = {
  "win32-x64-msvc": { os: "win32", cpu: "x64", file: "native.dll" },
  "linux-x64-gnu": { os: "linux", cpu: "x64", libc: "glibc", file: "libnative.so" },
  "linux-arm64-gnu": { os: "linux", cpu: "arm64", libc: "glibc", file: "libnative.so" },
  "darwin-aarch64": { os: "darwin", cpu: "arm64", file: "libnative.dylib" },
  "darwin-x64": { os: "darwin", cpu: "x64", file: "libnative.dylib" },
};

function parseArgs(argv: string[]): { src: string; triple: string } {
  let src: string | undefined;
  let triple: string | undefined;
  for (let i = 0; i < argv.length; i++) {
    if (argv[i] === "--src") {
      src = argv[i + 1];
      i++;
    } else if (argv[i] === "--triple") {
      triple = argv[i + 1];
      i++;
    }
  }
  if (src === undefined || triple === undefined) {
    const supported = Object.keys(PLATFORMS).join(", ");
    throw new Error(`usage: bun scripts/pack-platforms.ts --src <binary> --triple <${supported}>`);
  }
  return { src, triple };
}

const { src, triple } = parseArgs(process.argv.slice(2));
const platform = PLATFORMS[triple];
if (platform === undefined) {
  throw new Error(
    `unknown triple ${triple} (shipped: ${Object.keys(PLATFORMS).join(", ")})`,
  );
}

// The main package's name/version drive the platform package's identity
// (napi-rs convention: `<base>-<triple>`, ONE shared version).
const main = JSON.parse(
  readFileSync(join(import.meta.dir, "..", "package.json"), "utf8"),
) as { name: string; version: string; license?: string };
const base = main.name; // includes the scope, e.g. "@z2net/native-template"
const scopeless = base.includes("/") ? base.split("/")[1] ?? base : base;
const pkgName = `${base}-${triple}`;
const outDir = join(import.meta.dir, "..", "platform", `${scopeless}-${triple}`);

mkdirSync(outDir, { recursive: true });

// The binary travels under the convention file name; keep a stable
// copy (never move - the source stays for the other triples).
copyFileSync(src, join(outDir, platform.file));

const pkg = {
  name: pkgName,
  version: main.version,
  description: `${base} native binary (${triple})`,
  license: main.license ?? "MIT",
  main: "index.js",
  os: [platform.os],
  cpu: [platform.cpu],
  ...(platform.libc === undefined ? {} : { libc: [platform.libc] }),
};
writeFileSync(join(outDir, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`);

// The only contract `resolvePlatformBinary` relies on: the package
// main exports the ABSOLUTE binary path.
const shim = `"use strict";
const path = require("node:path");
module.exports = { path: path.join(__dirname, ${JSON.stringify(platform.file)}) };
`;
writeFileSync(join(outDir, "index.js"), shim);

console.log(`packed ${pkgName} -> ${outDir} (${platform.file})`);
