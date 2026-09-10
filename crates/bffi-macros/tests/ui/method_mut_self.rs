#![allow(unused)]
use bffi_macros::{bffi_class, bffi_impl};

#[bffi_class(tag = 0x0153, crate = "bffi")]
struct Mut {
    value: u32,
}

#[bffi_impl]
impl Mut {
    #[bffi_macros::bffi_constructor]
    pub fn new() -> Self {
        Self { value: 0 }
    }

    pub fn bump(&mut self) -> u32 {
        self.value + 1
    }
}

fn main() {}
