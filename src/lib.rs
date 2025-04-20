//! A simple yarn lockfile parser
//! Note - this is a slim public API file.

use napi_derive::napi;

/// Note - private modules not re-exported for usage
mod ident;
mod locator;
mod lockfile;
mod metadata;
mod package;

/// We re-export the parse module, for benchmarking etc
pub mod parse;

// NOTE: this is in lib.rs for now ,but eventually we may want to separate
// out the bindings, into its own crate
#[napi]
pub const fn parse(file_contents: String) -> String {
  // wow fast!!!!
  file_contents
}
