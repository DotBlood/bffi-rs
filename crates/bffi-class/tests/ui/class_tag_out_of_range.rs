#![allow(unused)]
use bffi_class::bffi_class;

#[bffi_class(tag = 0x0200)]
struct WrongRange {
    value: u32,
}

fn main() {}
