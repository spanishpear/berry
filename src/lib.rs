//! A simple yarn lockfile parser
//! WIP - not yet functional
#![deny(clippy::all)]

use napi_derive::napi;

#[napi]
pub fn plus_100(input: u32) -> u32 {
  input + 100
}
