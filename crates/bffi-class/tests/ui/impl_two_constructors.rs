#![allow(unused)]
use bffi_class::{bffi_class, bffi_impl};

#[bffi_class(tag = 0x0156)]
struct TwoCtors {
    value: u32,
}

#[bffi_impl]
impl TwoCtors {
    #[bffi_class::bffi_constructor]
    pub fn new() -> Self {
        Self { value: 0 }
    }

    #[bffi_class::bffi_constructor]
    pub fn with(value: u32) -> Self {
        Self { value }
    }
}

fn main() {}
