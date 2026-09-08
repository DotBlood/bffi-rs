/**
 * The public surface of `bffi-loader`: schema types, declaration
 * builder, wire codec, and the runtime primitives the generated
 * modules (and hand-rolled loaders) compose.
 */
export { ErrorCode, makeTakeError, type FfiLib, type FfiSymbol } from "./error.ts";
export { makeReadBuffer, makeFreeBuffer } from "./buffer.ts";
export {
  TAG_UNIT,
  TAG_I32,
  TAG_I64,
  TAG_F64,
  TAG_BOOL,
  TAG_STR,
  TAG_BYTES,
  decodeValue,
  encodeArgs,
  encodeValue,
  type WireValue,
} from "./wire.ts";
export {
  SCHEMA_VERSION,
  assertSchema,
  buildDeclarations,
  type AbiName,
  type ClassJson,
  type FieldJson,
  type FunctionJson,
  type ModuleJson,
  type OutName,
  type ParamJson,
  type RetAbiName,
  type RetJson,
  type TsName,
} from "./loader.ts";
export { pumpUntil, wrapTask } from "./async.ts";
export { createApi, createApiFromLib, type ApiOf, type ClassOf, type FnOf, type ParamsOf, type TsOf } from "./api.ts";
export {
  bindJsCallback,
  invokeCallback,
  revokeCallback,
  setJsThread,
  type CallbackSig,
  type CbType,
  type CbValue,
} from "./callbacks.ts";
