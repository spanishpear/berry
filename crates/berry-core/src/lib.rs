//! # Berry
//!
//! Berry is a library for parsing and manipulating Berry lockfiles.
//! It's still super WIP
//! ----
//! This project is not affiliated with Yarn or the Yarn team, but is a personal project
//! for my own learning and interest!
#![deny(clippy::all)]
pub mod ident;
pub mod locator;
pub mod lockfile;
pub mod metadata;
pub mod package;
pub mod parse;
