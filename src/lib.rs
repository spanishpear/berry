//! A simple yarn lockfile parser
//! WIP - not yet functional

use napi_derive::napi;

#[napi]
pub const fn plus_100(input: u32) -> u32 {
  input + 100
}

// test
