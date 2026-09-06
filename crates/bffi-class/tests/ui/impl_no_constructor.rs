#![allow(unused)]
use bffi_class::{bffi_class, bffi_impl};

#[bffi_class(tag = 0x0155)]
struct NoCtor {
    value: u32,
}

#[bffi_impl]
impl NoCtor {
    pub fn get(&self) -> u32 {
        self.value
    }
}

fn main() {}
