#![allow(unused)]
use bffi_class::{bffi_class, bffi_impl};

#[bffi_class(tag = 0x0154)]
struct Unsupported {
    value: u32,
}

#[bffi_impl]
impl Unsupported {
    #[bffi_class::bffi_constructor]
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn maybe(&self) -> Option<i32> {
        None
    }
}

fn main() {}
