/**
 * End-to-end test of the FULL `@z2net/bffi` pipeline on the sqlite
 * example: ONE call - cargo build -> loader JSON -> api.gen
 * generation -> binary resolution -> dlopen - then a real database
 * workload (open/exec/query/close + error paths).
 *
 * Run with `bun test examples/sqlite`.
 */
import { afterAll, describe, expect, test } from "bun:test";

import { bffi } from "@z2net/bffi";
import type { Api } from "../.bffi/api.gen.ts";

let api: Api;

afterAll(async () => {
  // Connections are closed individually below; nothing global to
  // release here.
});

describe("sqlite through the full pipeline", () => {
  test("the pipeline builds, generates and loads the module", async () => {
    api = await bffi({ config: `${import.meta.dir}/../.bffi/bffi.json` });
    expect(api).toBeTypeOf("object");
  });

  test("sqlite_version returns the engine version", () => {
    expect(api.sqlite_version().length).toBeGreaterThan(0);
  });

  test("open/exec/query round-trip a real table", () => {
    const handle = api.open(":memory:");
    expect(handle).toBeTypeOf("bigint");

    api.exec(handle, "CREATE TABLE t (id INTEGER, name TEXT)");
    api.exec(handle, "INSERT INTO t VALUES (1, 'ada'), (2, 'grace')");

    const rows: { id: string; name: string }[] = JSON.parse(
      api.query(handle, "SELECT id, name FROM t ORDER BY id"),
    );
    expect(rows).toEqual([
      { id: "1", name: "ada" },
      { id: "2", name: "grace" },
    ]);

    expect(api.close(handle)).toBeUndefined();
  });

  test("SQL errors surface as thrown Errors with the cause", () => {
    const handle = api.open(":memory:");
    expect(() =>
      api.exec(handle, "THIS IS NOT SQL"),
    ).toThrow();
    api.close(handle);
  });

  test("a closed handle is rejected (InvalidHandle)", () => {
    const handle = api.open(":memory:");
    api.close(handle);
    expect(() => api.exec(handle, "SELECT 1")).toThrow(/bad handle/);
  });
});
