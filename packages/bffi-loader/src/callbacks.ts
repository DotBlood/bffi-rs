/**
 * The JS-side callback surface over the generic callback ABI
 * (`bffi_callback_abi!()` exports; stage T4 of P4). The types are
 * final; the implementations land together with the Rust side.
 */

/** A callback value type (the `bffi-callback` `ValueType` matrix). */
export type CbType = "i32" | "i64" | "f64" | "bool";

/** A declared callback signature: return type plus parameter types. */
export interface CallbackSig {
  ret: CbType;
  params: CbType[];
}

/** The value a callback passes to or receives from the native side. */
export type CbValue = number | bigint | boolean;
