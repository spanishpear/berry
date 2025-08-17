//! # Berry
//!
//! Berry is a library for parsing and manipulating Berry lockfiles.
//! It's still super WIP - still trying to figure out if i use `berry-core` or `berry` on crates.io
#![deny(clippy::all)]
pub mod ident;
pub mod locator;
pub mod lockfile;
pub mod metadata;
pub mod package;
pub mod parse;
