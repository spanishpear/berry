use nom::IResult;
use nom::{
  Parser,
  branch::alt,
  bytes::complete::{tag, take_until, take_while1},
  character::complete::{alphanumeric1, char, space0},
  combinator::{opt, recognize},
  multi::separated_list0,
  sequence::{delimited, preceded, tuple},
};

use crate::ident::{Descriptor, Ident};
use crate::lockfile::{Lockfile, parse_metadata, parse_yarn_header};
use crate::package::Package;

/// Entrypoint for parsing a yarn lockfile
pub fn parse_lockfile(file_contents: &str) -> IResult<&str, Lockfile> {
  let (rest, (_, _)) = parse_yarn_header(file_contents)?;
  let (_rest, metadata) = parse_metadata(rest)?;

  dbg!(&metadata);

  todo!("actually parse the lockfile");
}

/// Parse a single package entry from the lockfile
/// Example input:
/// ```
/// "debug@npm:1.0.0":
///   version: 1.0.0
///   resolution: "debug@npm:1.0.0"
///   dependencies:
///     ms: 0.6.2
///   checksum: edfec8784737afbeea43cc78c3f56c33b88d3e751cc7220ae7a1c5370ff099e7352703275bdb56ea9967f92961231ce0625f8234d82259047303849671153f03
///   languageName: node
///   linkType: hard
/// ```
pub fn parse_package_entry(_input: &str) -> IResult<&str, (Descriptor, Package)> {
  todo!("implement package entry parsing")
}

/// Parse a package descriptor line like: "debug@npm:1.0.0":
pub fn parse_descriptor_line(input: &str) -> IResult<&str, Descriptor> {
  let (rest, descriptor_string) =
    delimited(char('"'), take_until("\":"), tag("\":")).parse(input)?;

  // Parse the descriptor string: name@protocol:range
  // Examples:
  // - debug@npm:1.0.0
  // - @babel/code-frame@npm:7.12.11
  // - a@workspace:packages/a

  let (remaining, (name_part, _, protocol, _, range_part)) = (
    parse_package_name, // Can be scoped like @babel/code-frame or simple like debug
    char('@'),
    parse_protocol, // npm, workspace, etc.
    char(':'),
    take_while1(|c: char| c != '"'), // The range/version part
  )
    .parse(descriptor_string)?;

  assert_eq!(remaining, "", "Should consume entire descriptor string");

  // Parse the name part to extract scope and name
  let ident = if name_part.starts_with('@') {
    // Scoped package: @babel/code-frame
    let parts: Vec<&str> = name_part[1..].splitn(2, '/').collect();
    if parts.len() == 2 {
      Ident::new(Some(format!("@{}", parts[0])), parts[1].to_string())
    } else {
      // Malformed scoped package, treat as simple name
      Ident::new(None, name_part.to_string())
    }
  } else {
    // Simple package: debug
    Ident::new(None, name_part.to_string())
  };

  // Combine protocol and range for the descriptor range
  let full_range = format!("{}:{}", protocol, range_part);

  Ok((rest, Descriptor::new(ident, full_range)))
}

/// Parse a package name, which can be scoped (@babel/code-frame) or simple (debug)
fn parse_package_name(input: &str) -> IResult<&str, &str> {
  alt((
    // Scoped package: @scope/name
    recognize((
      char('@'),
      take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
      char('/'),
      take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
    )),
    // Simple package name
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
  ))
  .parse(input)
}

/// Parse protocol part like npm, workspace, git, etc.
fn parse_protocol(input: &str) -> IResult<&str, &str> {
  take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_').parse(input)
}

/// Parse indented key-value properties for a package
pub fn parse_package_properties(_input: &str) -> IResult<&str, Package> {
  todo!("implement package properties parsing")
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_parse_descriptor_line_simple() {
    let input = r#""debug@npm:1.0.0":"#;
    let result = parse_descriptor_line(input);

    assert!(result.is_ok(), "Should successfully parse descriptor line");
    let (remaining, descriptor) = result.unwrap();
    assert_eq!(remaining, "");

    // For now, we'll just verify it doesn't panic
    // TODO: Add more specific assertions once we implement the parsing
  }

  #[test]
  fn test_parse_descriptor_line_scoped_package() {
    let input = r#""@babel/code-frame@npm:7.12.11":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse scoped package descriptor"
    );
    let (remaining, _descriptor) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_descriptor_line_workspace() {
    let input = r#""a@workspace:packages/a":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse workspace descriptor"
    );
    let (remaining, _descriptor) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_package_properties_minimal() {
    let input = r#"  version: 1.0.0
  resolution: "debug@npm:1.0.0"
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse minimal package properties"
    );
    let (remaining, _package) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_package_properties_with_dependencies() {
    let input = r#"  version: 1.0.0
  resolution: "debug@npm:1.0.0"
  dependencies:
    ms: 0.6.2
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with dependencies"
    );
    let (remaining, _package) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_package_properties_with_checksum() {
    let input = r#"  version: 1.0.0
  resolution: "debug@npm:1.0.0"
  checksum: edfec8784737afbeea43cc78c3f56c33b88d3e751cc7220ae7a1c5370ff099e7352703275bdb56ea9967f92961231ce0625f8234d82259047303849671153f03
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with checksum"
    );
    let (remaining, _package) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_full_package_entry() {
    let input = r#""debug@npm:1.0.0":
  version: 1.0.0
  resolution: "debug@npm:1.0.0"
  dependencies:
    ms: 0.6.2
  checksum: edfec8784737afbeea43cc78c3f56c33b88d3e751cc7220ae7a1c5370ff099e7352703275bdb56ea9967f92961231ce0625f8234d82259047303849671153f03
  languageName: node
  linkType: hard

"#;
    let result = parse_package_entry(input);

    assert!(
      result.is_ok(),
      "Should successfully parse complete package entry"
    );
    let (remaining, (_descriptor, _package)) = result.unwrap();
    assert_eq!(remaining, "");
  }

  #[test]
  fn test_parse_workspace_package_entry() {
    let input = r#""a@workspace:packages/a":
  version: 0.0.0-use.local
  resolution: "a@workspace:packages/a"
  languageName: unknown
  linkType: soft

"#;
    let result = parse_package_entry(input);

    assert!(
      result.is_ok(),
      "Should successfully parse workspace package entry"
    );
    let (remaining, (_descriptor, _package)) = result.unwrap();
    assert_eq!(remaining, "");
  }
}
