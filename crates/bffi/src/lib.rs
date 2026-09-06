//! # bffi
//!
//! The public facade of the bffi-rs framework: one dependency giving
//! the whole stack (DESIGN §5), with [`unsafe_zero_copy`] as the
//! single zero-copy door (DESIGN §6.3: "zero-copy is allowed only
//! through `bffi::unsafe_zero_copy`").
//!
//! ## What is re-exported
//!
//! | Area | Names |
//! | ---- | ----- |
//! | core | [`Handle`], [`TypeTag`], [`Registry`], [`BffiError`], [`ErrorCode`], `catch_panic`/`run_extern_body`, the TLS last-error pair |
//! | errors | [`JsErrorName`], [`JsErrorShape`], the code -> JS-constructor mapping |
//! | types | [`JsNumber`], [`CopiedBuf`], `str_view`/`buf_view`, the string converters |
//! | objects | [`ObjectWrap`], [`ObjectError`], the tag helpers |
//! | callbacks | `register`/`invoke`/`revoke`, `bind_js_callback`, the JS-thread gate |
//! | dts | the full IR + `render` + `sanitize` |
//! | macros | `#[bffi]`, `#[bffi_class]`, `#[bffi_impl]`, `#[bffi_constructor]`, `bffi_runtime_abi!` |
//! | event loop | `enqueue`/`marshal`/`run`/`stop`/`pump` |
//!
//! ## Macros keep their absolute paths
//!
//! The generated code of `#[bffi]` / `#[bffi_class]` /
//! `bffi_runtime_abi!` names `::bffi_core`, `::bffi_types`,
//! `::bffi_dts`, `::bffi_object` and `::bffi_build` - absolute paths
//! resolve through the DIRECT dependencies of the user crate, which a
//! re-export cannot provide. When you use the macros, keep those
//! crates in your `Cargo.toml`; the facade is the runtime umbrella
//! (see the example below).
//!
//! ## Example
//!
//! ```
//! use bffi::{CopiedBuf, ErrorCode, Handle, ObjectWrap, TypeTag};
//!
//! const SESSION: TypeTag = TypeTag(0x0160);
//!
//! // Objects: wrap, read, release - through the facade types.
//! let wrap = ObjectWrap::<u32>::new(SESSION).expect("tag claimed once");
//! let handle = wrap.wrap(42).expect("room");
//! assert_eq!(*wrap.get(handle).expect("live"), 42);
//!
//! // Copy by default everywhere else:
//! let copied = CopiedBuf::from_slice(b"copy by default");
//! assert_eq!(copied.as_slice(), b"copy by default");
//! let _ok: ErrorCode = ErrorCode::Ok;
//! let _null: Handle = Handle::NULL;
//! ```

// The workspace restriction lints (expect/unwrap/panic) target production
// code; tests assert invariants and intentionally trigger panics.
#![cfg_attr(test, allow(clippy::expect_used, clippy::panic, clippy::unwrap_used))]

pub use bffi_build::{BuildError, bffi_runtime_abi};
pub use bffi_callback::{
    CallbackError, CallbackSig, JsCallbackInfo, Value, ValueType, bind_js_callback,
    ensure_js_thread, invoke, js_callback, register, revoke, set_js_thread,
};
pub use bffi_class::{bffi_class, bffi_constructor, bffi_impl};
pub use bffi_core::{
    BffiError, ErrorCode, Handle, MAX_GENERATION, MAX_INDEX, Registry, RegistryError, TableError,
    TypeTag, catch_panic, panic_message, run_extern_body, run_extern_body_or, set_last_error,
    take_last_error,
};
pub use bffi_dts::{
    ClassDef, FieldDef, FunctionDef, MethodDef, ModuleDef, ParamDef, TsType, render, sanitize,
};
pub use bffi_error::{JsErrorExt, JsErrorName, JsErrorShape, js_error_name, take_last_error_shape};
pub use bffi_event_loop::{
    EventLoopError, Job, enqueue, executed_total, is_running, marshal, pending, pump, run, stop,
};
pub use bffi_macros::bffi;
pub use bffi_object::{ObjectError, ObjectWrap, TAG_MAX, TAG_MIN, tag_in_range};
pub use bffi_types::{
    ConversionError, CopiedBuf, JsNumber, buf_view, bytes_to_string, str_view, string_to_bytes,
};

/// THE single zero-copy door (DESIGN §6.3). Zero-copy is allowed only
/// through `bffi::unsafe_zero_copy`; everything else in this facade
/// copies by default.
///
/// The constructors are safe - they take `&[u8]` - but the returned
/// views borrow their input: the borrow checker keeps them from
/// outliving the FFI call. Never store a view in Rust state, and
/// assume JS may mutate the aliased memory at any time. The genuinely
/// unsafe `(ptr, len) -> &[u8]` step at the ABI lives in
/// `bffi-build`/the generated shims, not here.
///
/// ```
/// let text = bffi::str_view(b"hello").expect("valid utf-8");
/// assert_eq!(text.as_str(), "hello");
///
/// let bytes = [1_u8, 2, 3];
/// let view = bffi::buf_view(&bytes);
/// assert_eq!(view.as_slice(), [1, 2, 3]);
///
/// // The view types themselves are reachable only through the door:
/// let typed: bffi::unsafe_zero_copy::ZeroCopyStr<'_> = text;
/// let _buf: bffi::unsafe_zero_copy::ZeroCopyBuf<'_> = view;
/// ```
pub mod unsafe_zero_copy {
    // The view TYPES are exported only through this module - the
    // module name is the warning label. The constructor functions
    // (`str_view`/`buf_view`) are also available at the facade root
    // alongside the copying converters.
    pub use bffi_types::unsafe_zero_copy::{ZeroCopyBuf, ZeroCopyStr};
}
