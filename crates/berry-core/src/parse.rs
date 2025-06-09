use nom::IResult;
use nom::{
  Parser,
  branch::alt,
  bytes::complete::{is_not, tag, take_until, take_while1},
  character::complete::{char, newline, space1},
  combinator::{opt, recognize},
  multi::many0,
  sequence::delimited,
};

use crate::ident::{Descriptor, Ident};
use crate::lockfile::{Lockfile, parse_metadata, parse_yarn_header};
use crate::package::{LinkType, Package};

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
pub fn parse_package_entry(input: &str) -> IResult<&str, (Descriptor, Package)> {
  let (rest, descriptor) = parse_descriptor_line(input)?;
  let (rest, _) = newline.parse(rest)?; // consume newline after descriptor
  let (rest, package) = parse_package_properties(rest)?;

  Ok((rest, (descriptor, package)))
}

/// Parse a package descriptor line like: "debug@npm:1.0.0":
///
/// # Panics
///
/// This function will panic if the internal parser logic fails to consume the entire
/// descriptor string, which should not happen with valid input.
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
  let ident = name_part.strip_prefix('@').map_or_else(
    || Ident::new(None, name_part.to_string()),
    |stripped| {
      // Scoped package: @babel/code-frame
      let parts: Vec<&str> = stripped.splitn(2, '/').collect();
      if parts.len() == 2 {
        Ident::new(Some(format!("@{}", parts[0])), parts[1].to_string())
      } else {
        // Malformed scoped package, treat as simple name
        Ident::new(None, name_part.to_string())
      }
    },
  );

  // Combine protocol and range for the descriptor range
  let full_range = format!("{protocol}:{range_part}");

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
pub fn parse_package_properties(input: &str) -> IResult<&str, Package> {
  let (rest, properties) = many0(parse_property_line).parse(input)?;

  // Consume an optional trailing newline
  let (rest, _) = opt(newline).parse(rest)?;

  // Build the package from the parsed properties
  let mut package = Package::new("unknown".to_string(), LinkType::Hard);

  for (key, value) in properties {
    match key.as_str() {
      "version" => {
        package.version = Some(value.trim_matches('"').to_string());
      }
      "resolution" => {
        package.resolution = Some(value.trim_matches('"').to_string());
      }
      "languageName" => {
        package.language_name = crate::package::LanguageName::new(value.to_string());
      }
      "linkType" => {
        package.link_type = LinkType::try_from(value.as_str())
          .unwrap_or_else(|()| panic!("Invalid link type: {value}"));
      }
      "checksum" => {
        package.checksum = Some(value.to_string());
      }
      "dependencies" => {
        // For now, we'll skip parsing nested dependencies
        // This will be implemented in a future iteration
        todo!("parse nested dependencies");
      }
      _ => {
        // Skip unknown properties for now
        todo!("parse unknown properties");
      }
    }
  }

  Ok((rest, package))
}

/// Parse a single property line with 2-space indentation
/// Examples:
/// "  version: 1.0.0"
/// "  resolution: \"debug@npm:1.0.0\""
/// "  linkType: hard"
fn parse_property_line(input: &str) -> IResult<&str, (String, String)> {
  alt((parse_simple_property, parse_dependencies_block)).parse(input)
}

/// Parse a simple key-value property line
fn parse_simple_property(input: &str) -> IResult<&str, (String, String)> {
  let (rest, (_, key, _, _, value, _)) = (
    // FIXME: is this part of the spec?
    tag("  "), // 2-space indentation
    take_while1(|c: char| c.is_alphanumeric() || c == '_'),
    char(':'),
    space1,
    is_not("\r\n"), // Take everything until newline
    newline,
  )
    .parse(input)?;

  Ok((rest, (key.to_string(), value.to_string())))
}

/// Parse a dependencies block (for now, just consume it without parsing contents)
fn parse_dependencies_block(input: &str) -> IResult<&str, (String, String)> {
  let (rest, (_, _, _)) = (
    // FIXME: is this part of the spec?
    tag("  dependencies:"), // 2-space indented dependencies
    newline,
    many0(parse_dependency_line), // Parse nested dependency lines
  )
    .parse(input)?;

  // For now, return empty dependencies marker
  Ok((rest, ("dependencies".to_string(), "{}".to_string())))
}

/// Parse a single dependency line with 4-space indentation
/// Example: "    ms: 0.6.2"
fn parse_dependency_line(input: &str) -> IResult<&str, (String, String)> {
  let (rest, (_, dep_name, _, _, dep_range, _)) = (
    // FIXME: is this part of the spec?
    tag("    "), // 4-space indentation for dependencies
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
    char(':'),
    space1,
    is_not("\r\n"),
    newline,
  )
    .parse(input)?;

  Ok((rest, (dep_name.to_string(), dep_range.to_string())))
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

    // Verify the parsed descriptor
    assert_eq!(descriptor.ident().name(), "debug");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "npm:1.0.0");
  }

  #[test]
  fn test_parse_descriptor_line_scoped_package() {
    let input = r#""@babel/code-frame@npm:7.12.11":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse scoped package descriptor"
    );
    let (remaining, descriptor) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor
    assert_eq!(descriptor.ident().name(), "code-frame");
    assert_eq!(descriptor.ident().scope(), Some("@babel"));
    assert_eq!(descriptor.range(), "npm:7.12.11");
  }

  #[test]
  fn test_parse_descriptor_line_workspace() {
    let input = r#""a@workspace:packages/a":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse workspace descriptor"
    );
    let (remaining, descriptor) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor
    assert_eq!(descriptor.ident().name(), "a");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "workspace:packages/a");
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
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed package properties
    assert_eq!(package.version, Some("1.0.0".to_string()));
    assert_eq!(package.resolution, Some("debug@npm:1.0.0".to_string()));
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
    assert_eq!(package.checksum, None);
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
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed package properties
    assert_eq!(package.version, Some("1.0.0".to_string()));
    assert_eq!(package.resolution, Some("debug@npm:1.0.0".to_string()));
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
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
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed package properties
    assert_eq!(package.version, Some("1.0.0".to_string()));
    assert_eq!(package.resolution, Some("debug@npm:1.0.0".to_string()));
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
    assert_eq!(package.checksum, Some("edfec8784737afbeea43cc78c3f56c33b88d3e751cc7220ae7a1c5370ff099e7352703275bdb56ea9967f92961231ce0625f8234d82259047303849671153f03".to_string()));
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
    let (remaining, (descriptor, package)) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor
    assert_eq!(descriptor.ident().name(), "debug");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "npm:1.0.0");

    // Verify the parsed package
    assert_eq!(package.version, Some("1.0.0".to_string()));
    assert_eq!(package.resolution, Some("debug@npm:1.0.0".to_string()));
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
    assert_eq!(package.checksum, Some("edfec8784737afbeea43cc78c3f56c33b88d3e751cc7220ae7a1c5370ff099e7352703275bdb56ea9967f92961231ce0625f8234d82259047303849671153f03".to_string()));
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
    let (remaining, (descriptor, package)) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor
    assert_eq!(descriptor.ident().name(), "a");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "workspace:packages/a");

    // Verify the parsed package
    assert_eq!(package.version, Some("0.0.0-use.local".to_string()));
    assert_eq!(
      package.resolution,
      Some("a@workspace:packages/a".to_string())
    );
    assert_eq!(package.language_name.as_ref(), "unknown");
    assert_eq!(package.link_type, LinkType::Soft);
    assert_eq!(package.checksum, None);
  }
}
