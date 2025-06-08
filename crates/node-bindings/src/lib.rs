//! A simple yarn lockfile parser
//! Note - this is a slim public API file.

use napi::bindgen_prelude::Buffer;
use napi_derive::napi;

// NOTE: this is in lib.rs for now ,but eventually we may want to separate
// out the bindings, into its own crate
#[napi]
pub const fn parse(file_contents: Buffer) -> Buffer {
  // wow fast!!!!
  file_contents
}
