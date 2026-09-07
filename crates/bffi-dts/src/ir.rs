//! The static intermediate representation (IR) of TypeScript
//! declarations.
//!
//! Descriptors are plain `Copy` values over `&'static` slices so the
//! `#[bffi]` / `#[bffi_class]` macros can emit them as constants and
//! the renderer can consume them without allocation.
//!
//! # Canonical Rust -> TsType table
//!
//! The macro-side classification mirrors this table (see also the
//! type matrix in the `bffi-macros` README):
//!
//! | Rust                                   | TsType      |
//! |----------------------------------------|-------------|
//! | `i8` `i16` `i32` `u8` `u16` `u32` `f32` `f64` | `Number` |
//! | `i64` `u64` (including handles)        | `BigInt`    |
//! | `bool`                                 | `Boolean`   |
//! | `&str`, `String`                       | `String`    |
//! | `Vec<u8>`, `CopiedBuf`                 | `Uint8Array`|
//! | `Option<String>`                       | `NullableString` |
//! | `Option<Vec<u8>>`, `Option<CopiedBuf>` | `NullableUint8Array` |
//! | `()`                                   | `Void`      |
//!
//! The `Nullable*` variants render with `| null` (e.g.
//! `"string | null"`), so the nullability of an `Option` return is
//! part of the type contract itself. They are flat (payload-free)
//! variants on purpose: the IR must stay `Copy` so the macros can
//! emit descriptors as constants (`Box::new` is not allowed in const
//! context).
//!
//! The same flatness rule produced the `Promise*` variants
//! (`PromiseVoid`/`Number`/`BigInt`/`Boolean`/`String`/`Uint8Array`),
//! emitted by `#[bffi_async]`: an async export returns a task handle
//! at the ABI level and a `Promise<T>` at the JS level - the
//! descriptor describes the JS-level contract.

/// A TypeScript type referenced by a declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TsType {
    /// The TypeScript `number` type.
    Number,
    /// The TypeScript `bigint` type.
    BigInt,
    /// The TypeScript `boolean` type.
    Boolean,
    /// The TypeScript `string` type.
    String,
    /// The TypeScript `Uint8Array` type.
    Uint8Array,
    /// The TypeScript `string | null` type (`Option<String>` returns).
    NullableString,
    /// The TypeScript `Uint8Array | null` type (`Option<Vec<u8>>` /
    /// `Option<CopiedBuf>` returns).
    NullableUint8Array,
    /// The TypeScript `Promise<void>` type (`#[bffi_async]` exports).
    PromiseVoid,
    /// The TypeScript `Promise<number>` type (`#[bffi_async]` returns
    /// of the number-ish primitives).
    PromiseNumber,
    /// The TypeScript `Promise<bigint>` type (`#[bffi_async]` returns
    /// of `i64`/`u64`).
    PromiseBigInt,
    /// The TypeScript `Promise<boolean>` type (`#[bffi_async]` returns
    /// of `bool`).
    PromiseBoolean,
    /// The TypeScript `Promise<string>` type (`#[bffi_async]` returns
    /// of `String`).
    PromiseString,
    /// The TypeScript `Promise<Uint8Array>` type (`#[bffi_async]`
    /// returns of `Vec<u8>` / `CopiedBuf`).
    PromiseUint8Array,
    /// The TypeScript `void` type.
    Void,
}

impl TsType {
    /// The TypeScript name of this type, as written in a `.d.ts`
    /// file (e.g. `"number"`, `"Uint8Array"`, `"void"`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Number => "number",
            Self::BigInt => "bigint",
            Self::Boolean => "boolean",
            Self::String => "string",
            Self::Uint8Array => "Uint8Array",
            Self::NullableString => "string | null",
            Self::NullableUint8Array => "Uint8Array | null",
            Self::PromiseVoid => "Promise<void>",
            Self::PromiseNumber => "Promise<number>",
            Self::PromiseBigInt => "Promise<bigint>",
            Self::PromiseBoolean => "Promise<boolean>",
            Self::PromiseString => "Promise<string>",
            Self::PromiseUint8Array => "Promise<Uint8Array>",
            Self::Void => "void",
        }
    }
}

/// A single function parameter: its JS-visible name and type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ParamDef {
    /// The parameter name as it appears in the generated declaration.
    pub name: &'static str,
    /// The parameter type.
    pub ty: TsType,
}

/// A native function exposed to JavaScript.
///
/// `export_name` follows the `bffi_` + `js_name` convention and is
/// consumed by `bffi-build` to link the C ABI symbol; it is never
/// rendered into the `.d.ts` output. `docs` feeds the JSDoc block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FunctionDef {
    /// The function name as seen from JavaScript.
    pub js_name: &'static str,
    /// The C ABI export symbol (`bffi_` + `js_name`); build-side only.
    pub export_name: &'static str,
    /// Doc comment lines, rendered as a JSDoc block.
    pub docs: &'static [&'static str],
    /// The parameters, in declaration order.
    pub params: &'static [ParamDef],
    /// The return type.
    pub ret: TsType,
}

/// A named module grouping the native functions and classes it
/// exports.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ModuleDef {
    /// The module name as seen from JavaScript.
    pub name: &'static str,
    /// The functions exported by this module.
    pub fns: &'static [FunctionDef],
    /// The classes exported by this module.
    pub classes: &'static [ClassDef],
}

/// A class constructor or method: the `FunctionDef` shape as seen
/// from inside a class body (`params` exclude the receiver; the
/// export symbol follows `bffi_<class>_<method>`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MethodDef {
    /// The method name as seen from JavaScript; the constructor uses
    /// the literal `constructor`.
    pub js_name: &'static str,
    /// The C ABI export symbol; build-side only, never rendered.
    pub export_name: &'static str,
    /// Doc comment lines, rendered as a JSDoc block.
    pub docs: &'static [&'static str],
    /// The parameters, in declaration order.
    pub params: &'static [ParamDef],
    /// The return type.
    pub ret: TsType,
}

/// A class field exposed as a read-only getter (P2 v1: getters only -
/// the Arc-based ownership model has no safe setter).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldDef {
    /// The field name as seen from JavaScript.
    pub js_name: &'static str,
    /// Doc comment lines, rendered as a JSDoc block.
    pub docs: &'static [&'static str],
    /// The field type.
    pub ty: TsType,
}

/// A native class exposed to JavaScript: generated by
/// `#[bffi_class]` over an `ObjectWrap`-backed Rust struct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClassDef {
    /// The class name as seen from JavaScript.
    pub js_name: &'static str,
    /// Doc comment lines, rendered as a JSDoc block.
    pub docs: &'static [&'static str],
    /// The constructor declaration (exactly one in P2 v1).
    pub constructor: MethodDef,
    /// The read-only field getters, in declaration order.
    pub fields: &'static [FieldDef],
    /// The methods, in declaration order.
    pub methods: &'static [MethodDef],
}

#[cfg(test)]
mod tests {
    use super::{FunctionDef, ModuleDef, ParamDef, TsType};

    #[test]
    fn ts_type_as_str_covers_all_variants() {
        assert_eq!(TsType::Number.as_str(), "number");
        assert_eq!(TsType::BigInt.as_str(), "bigint");
        assert_eq!(TsType::Boolean.as_str(), "boolean");
        assert_eq!(TsType::String.as_str(), "string");
        assert_eq!(TsType::Uint8Array.as_str(), "Uint8Array");
        assert_eq!(TsType::NullableString.as_str(), "string | null");
        assert_eq!(TsType::NullableUint8Array.as_str(), "Uint8Array | null");
        assert_eq!(TsType::Void.as_str(), "void");
    }

    #[test]
    fn param_def_compares_by_value() {
        let param = ParamDef {
            name: "value",
            ty: TsType::Number,
        };
        let same = ParamDef {
            name: "value",
            ty: TsType::Number,
        };
        let different_name = ParamDef {
            name: "other",
            ty: TsType::Number,
        };
        let different_ty = ParamDef {
            name: "value",
            ty: TsType::String,
        };
        assert_eq!(param, same, "identical params must compare equal");
        assert_ne!(param, different_name, "different names must not be equal");
        assert_ne!(param, different_ty, "different types must not be equal");
    }

    #[test]
    fn function_def_compares_by_value() {
        static DOCS: &[&str] = &["Adds two numbers."];
        static PARAMS: [ParamDef; 1] = [ParamDef {
            name: "a",
            ty: TsType::Number,
        }];
        static DIFFERENT_DOCS: &[&str] = &["Subtracts."];

        let function = FunctionDef {
            js_name: "add",
            export_name: "bffi_add",
            docs: DOCS,
            params: &PARAMS,
            ret: TsType::Number,
        };
        let same = FunctionDef {
            js_name: "add",
            export_name: "bffi_add",
            docs: DOCS,
            params: &PARAMS,
            ret: TsType::Number,
        };
        let different_js_name = FunctionDef {
            js_name: "sub",
            export_name: "bffi_add",
            docs: DOCS,
            params: &PARAMS,
            ret: TsType::Number,
        };
        let with_different_docs = FunctionDef {
            js_name: "add",
            export_name: "bffi_add",
            docs: DIFFERENT_DOCS,
            params: &PARAMS,
            ret: TsType::Number,
        };
        assert_eq!(function, same, "identical defs must compare equal");
        assert_ne!(
            function, different_js_name,
            "different js_name must not be equal"
        );
        assert_ne!(
            function, with_different_docs,
            "different docs must not be equal"
        );
    }

    #[test]
    fn module_def_compares_by_value() {
        static FNS_A: [FunctionDef; 1] = [FunctionDef {
            js_name: "add",
            export_name: "bffi_add",
            docs: &[],
            params: &[],
            ret: TsType::Number,
        }];
        static FNS_B: [FunctionDef; 1] = [FunctionDef {
            js_name: "sub",
            export_name: "bffi_sub",
            docs: &[],
            params: &[],
            ret: TsType::Void,
        }];

        let module = ModuleDef {
            name: "native",
            fns: &FNS_A,
            classes: &[],
        };
        let same = ModuleDef {
            name: "native",
            fns: &FNS_A,
            classes: &[],
        };
        let different = ModuleDef {
            name: "native",
            fns: &FNS_B,
            classes: &[],
        };
        assert_eq!(module, same, "identical modules must compare equal");
        assert_ne!(module, different, "different fns must not be equal");
    }

    #[test]
    fn descriptors_are_copy_and_static() {
        fn assert_copy<T: Copy>() {}

        assert_copy::<TsType>();
        assert_copy::<ParamDef>();
        assert_copy::<FunctionDef>();
        assert_copy::<ModuleDef>();

        assert_eq!(FNS.len(), 2);
        assert_eq!(MODULE.fns.len(), 2);
        assert_eq!(MODULE.name, "native");
        assert_eq!(MODULE.fns[0].ret, TsType::BigInt);
        assert_eq!(MODULE.fns[1].ret, TsType::NullableUint8Array);
    }

    #[test]
    fn nullable_string_renders_with_null() {
        static NULLABLE_FNS: &[FunctionDef] = &[FunctionDef {
            js_name: "maybe_name",
            export_name: "bffi_maybe_name",
            docs: &[],
            params: &[],
            ret: TsType::NullableString,
        }];
        let module = ModuleDef {
            name: "native",
            fns: NULLABLE_FNS,
            classes: &[],
        };
        let rendered = crate::render::render(&module);
        assert!(rendered.contains("export function maybe_name(): string | null;"));
    }

    static FNS: &[FunctionDef] = &[
        FunctionDef {
            js_name: "tick",
            export_name: "bffi_tick",
            docs: &[],
            params: &[],
            ret: TsType::BigInt,
        },
        FunctionDef {
            js_name: "peek",
            export_name: "bffi_peek",
            docs: &[],
            params: &[],
            ret: TsType::NullableUint8Array,
        },
    ];

    static MODULE: ModuleDef = ModuleDef {
        name: "native",
        fns: FNS,
        classes: &[],
    };
}
