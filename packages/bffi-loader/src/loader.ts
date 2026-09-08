/**
 * The loader-schema types (version 1, emitted by
 * `bffi_build::loader_json`) and the dlopen declaration builder that
 * turns the ABI view back into `bun:ffi` shapes.
 *
 * Canonical ABI names vs `bun:ffi` spellings differ in exactly two
 * places: `bool` crosses dlopen as `"u8"` (JS coerces `0`/`1`), and a
 * `ptr_len` parameter expands into the `("ptr", "u64")` pair.
 */

/** Schema version this loader accepts. */
export const SCHEMA_VERSION = 1;

/** The bun:ffi declaration spelling: the FFIType enum or one of its
 * runtime string names (`"u32"`, `"cstring"`, ...). */
export type FfiType = import("bun:ffi").FFITypeOrString;

/** The canonical ABI name of one parameter entry. */
export type AbiName =
  | "i8"
  | "i16"
  | "i32"
  | "u8"
  | "u16"
  | "u32"
  | "f32"
  | "f64"
  | "i64"
  | "u64"
  | "bool"
  | "cstring"
  | "ptr_len";

/** The out-slot name: a primitive width or the shared handle slot. */
export type OutName =
  | AbiName
  | "handle";

/** The return transport: a primitive width, a transient-buffer
 * handle, an async task handle, or the unit return. */
export type RetAbiName =
  | OutName
  | "buffer"
  | "task"
  | "void";

/** The TypeScript type name as written by `bffi-dts`. */
export type TsName =
  | "number"
  | "bigint"
  | "boolean"
  | "string"
  | "Uint8Array"
  | "string | null"
  | "Uint8Array | null"
  | "void"
  | `Promise<${string}>`;

export interface ParamJson {
  name: string;
  ts: TsName;
  abi: AbiName;
}

export interface RetJson {
  ts: TsName;
  abi: RetAbiName;
}

export interface FunctionJson {
  name: string;
  export: string;
  docs: string[];
  params: ParamJson[];
  ret: RetJson;
  /** The out slot; absent for the unit return. */
  out?: OutName;
}

export interface FieldJson {
  name: string;
  export: string;
  docs: string[];
  ts: TsName;
  out: OutName;
}

export interface ClassJson {
  name: string;
  /** The generated release shim: `bffi_<name>_release`. */
  release: string;
  docs: string[];
  constructor: FunctionJson;
  fields: FieldJson[];
  methods: FunctionJson[];
}

export interface ModuleJson {
  bffi: number;
  module: string;
  functions: FunctionJson[];
  classes: ClassJson[];
}

/** Validates the schema header: unknown versions are rejected. */
export function assertSchema(json: ModuleJson): void {
  if (json.bffi !== SCHEMA_VERSION) {
    throw new Error(
      `unsupported bffi loader schema: ${String(json.bffi)} (expected ${SCHEMA_VERSION})`,
    );
  }
}

/** The `bun:ffi` argument spelling of one ABI parameter. The FFIType
 * enum members carry these literal names, so the literals type-check
 * contextually. */
function ffiArg(abi: AbiName): FfiType[] {
  switch (abi) {
    case "i8":
      return ["i8"];
    case "i16":
      return ["i16"];
    case "i32":
      return ["i32"];
    case "u8":
      return ["u8"];
    case "u16":
      return ["u16"];
    case "u32":
      return ["u32"];
    case "f32":
      return ["f32"];
    case "f64":
      return ["f64"];
    case "i64":
      return ["i64"];
    case "u64":
      return ["u64"];
    case "bool":
      return ["u8"];
    case "cstring":
      return ["cstring"];
    case "ptr_len":
      return ["ptr", "u64"];
  }
}

/**
 * Builds the `dlopen` declarations object for a module: every export
 * keyed by symbol name, `args` derived from the ABI parameter view
 * (the receiver handle of class members prepended by the caller) and
 * the trailing out-parameter, `returns` always the `u32` ErrorCode.
 */
export function buildDeclarations(json: ModuleJson): Record<string, {
  args: FfiType[];
  returns: FfiType;
}> {
  assertSchema(json);
  const declarations: Record<string, { args: FfiType[]; returns: FfiType }> = {};
  const add = (
    exportName: string,
    abi: { params: { abi: AbiName }[] },
    out: OutName | undefined,
  ): void => {
    if (exportName in declarations) {
      throw new Error(`duplicate export symbol: ${exportName}`);
    }
    const args = abi.params.flatMap((param) => ffiArg(param.abi));
    if (out !== undefined) {
      args.push("pointer");
    }
    declarations[exportName] = { args, returns: "u32" };
  };
  for (const fn of json.functions) {
    add(fn.export, fn, fn.out);
  }
  for (const cls of json.classes) {
    // Class members receive the instance handle first: one `u64`
    // argument prepended by the caller (CALLING-CONVENTION.md §7).
    declarations[cls.release] = { args: ["u64"], returns: "u32" };
    declarations[cls.constructor.export] = {
      args: cls.constructor.params.flatMap((param) => ffiArg(param.abi)).concat(
        cls.constructor.out === undefined ? [] : ["pointer"],
      ),
      returns: "u32",
    };
    for (const field of cls.fields) {
      if (field.export in declarations) {
        throw new Error(`duplicate export symbol: ${field.export}`);
      }
      declarations[field.export] = { args: ["u64", "pointer"], returns: "u32" };
    }
    for (const method of cls.methods) {
      if (method.export in declarations) {
        throw new Error(`duplicate export symbol: ${method.export}`);
      }
      const args = ["u64" as FfiType].concat(method.params.flatMap((param) => ffiArg(param.abi)));
      if (method.out !== undefined) {
        args.push("pointer");
      }
      declarations[method.export] = { args, returns: "u32" };
    }
  }
  return declarations;
}
