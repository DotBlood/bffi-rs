#![allow(unused)]
use bffi_macros::bffi_class;

#[bffi_class(tag = 0x0152)]
enum NotAStruct {
    A,
}

fn main() {}
