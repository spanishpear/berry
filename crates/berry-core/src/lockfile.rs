use crate::package::LockfileEntry;
use nom::{
  IResult, Parser,
  bytes::complete::{is_not, tag, take_while},
  character::complete::{char, newline, space1},
  sequence::{pair, preceded, separated_pair, terminated},
};

/// A serialized representation of a yarn lockfile.
#[derive(Debug)]
pub struct Lockfile {
  /// Lockfile version and cache key
  pub metadata: Metadata,
  /// The entries in the lockfile
  pub entries: Vec<LockfileEntry>,
}

/// The start of the metadata block
/// Typically at the start of the file
#[derive(Debug)]
#[allow(dead_code)]
pub struct Metadata {
  pub version: String,
  pub cache_key: String,
}

impl Metadata {
  pub fn new(version: String, cache_key: String) -> Self {
    Self { version, cache_key }
  }
}

/// A line of metadata is a key-value pair, with a space-based indent
/// e.g. `  version: 8`
pub(crate) fn parse_metadata_line(input: &str) -> IResult<&str, (&str, &str)> {
  terminated(
    preceded(
      space1,
      separated_pair(
        take_while(|c: char| c.is_alphabetic() || c == '_'),
        pair(char(':'), space1),
        is_not("\r\n"),
      ),
    ),
    newline,
  )
  .parse(input)
}

/// Parses the __metadata block of a yarn lockfile
/// e.g.
/// __metadata:
///   version: 8
///   cacheKey: 9
pub(crate) fn parse_metadata(input: &str) -> IResult<&str, Metadata> {
  let (rest, _) = terminated(tag("__metadata:"), newline).parse(input)?;
  let (rest, version_line) = parse_metadata_line(rest)?;
  let (rest, cache_key_line) = parse_metadata_line(rest)?;

  // todo(shrey): consider a more robust way to do this
  // where we dont rely on the ordering
  let version = version_line.1.trim_matches('"');
  let cache_key = cache_key_line.1.trim_matches('"');

  Ok((
    rest,
    Metadata::new(version.to_string(), cache_key.to_string()),
  ))
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_metadata() {
    let metadata_block = r#"__metadata:
  version: "8"
  cacheKey: "9"

"#;
    let result = parse_metadata(metadata_block);
    assert!(result.is_ok());

    let (rest, metadata) = result.unwrap();
    assert_eq!(rest, "\n");
    assert_eq!(metadata.version, "8");
    assert_eq!(metadata.cache_key, "9");
  }
}
