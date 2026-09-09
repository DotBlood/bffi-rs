/** `bffi pack --src <binary> --triple <t> [--name <base>] [--out <dir>]`:
 * assembles one platform npm package (pattern A) from a built
 * cdylib - the binary (artifact naming convention), a package.json
 * carrying os/cpu/libc, and an `index.js` shim exporting `{ path }`
 * (the only contract `resolvePlatformBinary` relies on). */
import { copyFileSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { joinOut, platformTriple } from "@z2net/bffi";
import { flagString, parseArgs } from "../args.ts";
import { EXIT, writeErr, writeOut } from "../output.ts";

export const usage =
  `bffi pack --src <binary> --triple <t> [--name <base>] [--out <dir>]`;

/** triple -> npm os/cpu/libc fields + the binary file name inside the
 * platform package. Extend the map when you add targets. */
const PLATFORMS: Record<
  string,
  { os: string; cpu: string; libc?: string; file: string }
> = {
  "win32-x64-msvc": { os: "win32", cpu: "x64", file: "native.dll" },
  "linux-x64-gnu": { os: "linux", cpu: "x64", libc: "glibc", file: "libnative.so" },
  "linux-arm64-gnu": { os: "linux", cpu: "arm64", libc: "glibc", file: "libnative.so" },
  "linux-x64-musl": { os: "linux", cpu: "x64", libc: "musl", file: "libnative.so" },
  "linux-arm64-musl": { os: "linux", cpu: "arm64", libc: "musl", file: "libnative.so" },
  "darwin-aarch64": { os: "darwin", cpu: "arm64", file: "libnative.dylib" },
  "darwin-x64": { os: "darwin", cpu: "x64", file: "libnative.dylib" },
};

export async function pack(argv: string[]): Promise<number> {
  const args = parseArgs(argv);
  const src = flagString(args, "src");
  const triple = flagString(args, "triple") ?? platformTriple();
  const base = flagString(args, "name") ?? "@z2net/native-template";
  const outDir = flagString(args, "out") ?? "platform";

  if (src === undefined) {
    writeErr(usage);
    return EXIT.usage;
  }
  const platform = PLATFORMS[triple];
  if (platform === undefined) {
    writeErr(
      `pack: unknown triple ${triple} (shipped: ${Object.keys(PLATFORMS).join(", ")})`,
    );
    return EXIT.fail;
  }

  const main = JSON.parse(
    readFileSync(join(process.cwd(), "package.json"), "utf8"),
  ) as { name: string; version: string; license?: string };
  const parts = base.split("/");
  const scopeless = parts.length > 1 ? parts[parts.length - 1] ?? base : base;
  const pkgName = `${base}-${triple}`;
  const pkgDir = joinOut(process.cwd(), join(outDir, `${scopeless}-${triple}`));

  mkdirSync(pkgDir, { recursive: true });
  copyFileSync(src, join(pkgDir, platform.file));

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
  writeFileSync(join(pkgDir, "package.json"), `${JSON.stringify(pkg, null, 2)}\n`);

  const shim = `"use strict";
const path = require("node:path");
module.exports = { path: path.join(__dirname, ${JSON.stringify(platform.file)}) };
`;
  writeFileSync(join(pkgDir, "index.js"), shim);

  writeOut(`packed ${pkgName} -> ${pkgDir} (${platform.file})`);
  return EXIT.ok;
}
