#![allow(unused)]
use bffi_macros::bffi_class;

#[bffi_class]
struct NoTag {
    value: u32,
}

fn main() {}
