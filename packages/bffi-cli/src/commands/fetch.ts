/** `bffi fetch --repo <owner/repo> --tag <v> --asset <file>
 * [--out <dir>]`: downloads a prebuilt binary (+ its `.sha256`
 * sidecar) from a GitHub Release and verifies the digest. */
import { joinOut } from "@z2net/bffi";
import { flagString, parseArgs } from "../args.ts";
import { EXIT, writeErr, writeOut } from "../output.ts";

export const usage =
  `bffi fetch --repo <owner/repo> --tag <v> --asset <file> [--out <dir>]`;

/** The command handler. Named `fetchCmd` so it does not shadow the
 * global `fetch` used for downloads. */
export async function fetchCmd(argv: string[]): Promise<number> {
  const args = parseArgs(argv);
  const repo = flagString(args, "repo");
  const tag = flagString(args, "tag");
  const asset = flagString(args, "asset");
  const outDir = flagString(args, "out") ?? joinOut("target", "bffi");

  if (repo === undefined || tag === undefined || asset === undefined) {
    writeErr(usage);
    return EXIT.usage;
  }

  const baseUrl = `https://github.com/${repo}/releases/download/${tag}`;
  const response = await fetch(`${baseUrl}/${asset}`);
  if (!response.ok) {
    writeErr(`fetch: ${baseUrl}/${asset} -> HTTP ${String(response.status)}`);
    return EXIT.fail;
  }
  const bytes = new Uint8Array(await response.arrayBuffer());

  // Digest check against the `<asset>.sha256` sidecar ("<hex>" or
  // "<hex>  <filename>").
  const hashResponse = await fetch(`${baseUrl}/${asset}.sha256`);
  if (hashResponse.ok) {
    const expected = (await hashResponse.text()).trim().split(/\s+/)[0] ?? "";
    const hasher = new Bun.CryptoHasher("sha256");
    hasher.update(bytes);
    const actual = hasher.digest("hex");
    if (actual !== expected) {
      writeErr(`fetch: sha256 mismatch (expected ${expected}, got ${actual})`);
      return EXIT.fail;
    }
  }

  const outPath = joinOut(process.cwd(), outDir, asset);
  await Bun.write(outPath, bytes);
  writeOut(`fetched ${asset} (${String(bytes.length)} bytes) -> ${outPath}`);
  return EXIT.ok;
}
