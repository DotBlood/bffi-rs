#![allow(unused)]
use bffi_macros::bffi;

struct S;

impl S {
    #[bffi]
    fn f(&self) {}
}

fn main() {}
