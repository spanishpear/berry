//! A simple yarn lockfile parser
//! Note - this is a slim public API file.

use napi_derive::napi;

/// Note - private modules not re-exported for usage
mod ident;
mod locator;
mod lockfile;
mod metadata;
mod package;
mod parse;

#[napi]
pub const fn parse(file_contents: String) -> String {
  file_contents
}
