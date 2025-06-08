use nom::IResult;

use crate::lockfile::{Lockfile, parse_metadata, parse_yarn_header};

/// Entrypoint for parsing a yarn lockfile
pub fn parse_lockfile(file_contents: &str) -> IResult<&str, Lockfile> {
  let (rest, (_, _)) = parse_yarn_header(file_contents)?;
  let (_rest, metadata) = parse_metadata(rest)?;

  dbg!(&metadata);

  todo!("actually parse the lockfile");
}
