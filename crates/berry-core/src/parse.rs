use nom::IResult;
use nom::{
  Parser,
  branch::alt,
  bytes::complete::{is_not, tag, take_until, take_while1},
  character::complete::{char, newline, space0, space1},
  combinator::{map, opt, recognize},
  multi::{many0, separated_list1, fold_many0},
  sequence::{delimited, tuple},
};

use crate::ident::{Descriptor, Ident};
use crate::lockfile::{Lockfile, parse_metadata, parse_yarn_header};
use crate::package::{LinkType, Package};

/// Parse just the package from a package entry, discarding the descriptor
fn parse_package_only(input: &str) -> IResult<&str, Package> {
  map(parse_package_entry, |(_, package)| package).parse(input)
}

/// Entrypoint for parsing a yarn lockfile
pub fn parse_lockfile(file_contents: &str) -> IResult<&str, Lockfile> {
  let (rest, (_, _)) = parse_yarn_header(file_contents)?;
  let (rest, metadata) = parse_metadata(rest)?;

  // Consume any blank lines after metadata
  let (rest, _) = opt(newline).parse(rest)?;

  // Parse all package entries, extracting just the Package from each entry
  let (rest, packages) = many0(parse_package_only).parse(rest)?;

  Ok((rest, Lockfile { metadata, entries: packages }))
}

/// Parse a single package entry from the lockfile
///
/// Example input:
///
/// ```text
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

/// Parse a package descriptor line like: "debug@npm:1.0.0": or "c@*, c@workspace:packages/c":
pub fn parse_descriptor_line(input: &str) -> IResult<&str, Descriptor> {
  let (rest, descriptor_string) =
    delimited(char('"'), take_until("\":"), tag("\":")).parse(input)?;

  // Parse comma-separated descriptors using nom combinators
  let (remaining, descriptors) = separated_list1(
    tuple((space0, char(','), space0)),
    parse_single_descriptor,
  ).parse(descriptor_string)?;

  assert_eq!(remaining, "", "Should consume entire descriptor string");

  // For now, return the first descriptor (future enhancement: handle multiple descriptors)
  let first_descriptor = descriptors.into_iter().next()
    .expect("separated_list1 should guarantee at least one descriptor");

  Ok((rest, first_descriptor))
}

/// Parse a single descriptor string like "debug@npm:1.0.0" or "c@*"
fn parse_single_descriptor(input: &str) -> IResult<&str, Descriptor> {
  // Try protocol:range format first (e.g., npm:1.0.0)
  if let Ok((remaining, (name_part, _, protocol, _, range))) = tuple((
    parse_package_name,
    char('@'),
    parse_protocol,
    char(':'),
    take_while1(|c: char| c != ',' && c != '"'),
  )).parse(input) {
    let ident = parse_name_to_ident(name_part);
    let full_range = format!("{}:{}", protocol, range);
    return Ok((remaining, Descriptor::new(ident, full_range)));
  }

  // Try simple range format (e.g., * for c@*)
  if let Ok((remaining, (name_part, _, range))) = tuple((
    parse_package_name,
    char('@'),
    take_while1(|c: char| c != ',' && c != '"'),
  )).parse(input) {
    let ident = parse_name_to_ident(name_part);
    return Ok((remaining, Descriptor::new(ident, range.to_string())));
  }

  Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Alt)))
}

/// Helper function to parse name part into Ident
fn parse_name_to_ident(name_part: &str) -> Ident {
  name_part.strip_prefix('@').map_or_else(
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
  )
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
    // non-scoped package name: debug
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_'),
  ))
  .parse(input)
}

/// Parse protocol part like npm, workspace, git, etc.
/// TODO: should we validate the protocol? e.g. npm, workspace, git, file, root, etc.
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

  for property_value in properties {
    match property_value {
      PropertyValue::Simple(key, value) => {
        match key {
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
            package.link_type = LinkType::try_from(value)
              .unwrap_or_else(|()| panic!("Invalid link type: {value}"));
          }
          "checksum" => {
            package.checksum = Some(value.to_string());
          }
          _ => {
            // Skip unknown properties gracefully
          }
        }
      }
      PropertyValue::Dependencies(dependencies) => {
        // Store the parsed dependencies in the package
        for (dep_name, dep_range) in dependencies {
          let ident = parse_dependency_name_to_ident(dep_name);
          let descriptor = Descriptor::new(ident, dep_range.to_string());
          package.dependencies.insert(descriptor.ident().clone(), descriptor);
        }
      }
      PropertyValue::PeerDependencies(peer_dependencies) => {
        // Store the parsed peer dependencies in the package
        for (dep_name, dep_range) in peer_dependencies {
          let ident = parse_dependency_name_to_ident(dep_name);
          let descriptor = Descriptor::new(ident, dep_range.to_string());
          package.peer_dependencies.insert(descriptor.ident().clone(), descriptor);
        }
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
fn parse_property_line(input: &str) -> IResult<&str, PropertyValue<'_>> {
  // Try simple property first
  if let Ok((rest, (key, value))) = parse_simple_property(input) {
    return Ok((rest, PropertyValue::Simple(key, value)));
  }

  // Try dependencies block
  if let Ok((rest, dependencies)) = parse_dependencies_block(input) {
    return Ok((rest, PropertyValue::Dependencies(dependencies)));
  }

  // Try peer dependencies block
  if let Ok((rest, peer_dependencies)) = parse_peer_dependencies_block(input) {
    return Ok((rest, PropertyValue::PeerDependencies(peer_dependencies)));
  }

  // If nothing matches, return an error
  Err(nom::Err::Error(nom::error::Error::new(input, nom::error::ErrorKind::Alt)))
}

/// Enum to represent different types of property values
#[derive(Debug)]
enum PropertyValue<'a> {
  Simple(&'a str, &'a str),
  Dependencies(Vec<(&'a str, &'a str)>),  // Use Vec instead of HashMap to avoid allocations
  PeerDependencies(Vec<(&'a str, &'a str)>),  // Use Vec instead of HashMap to avoid allocations
}

/// Parse a simple key-value property line
fn parse_simple_property(input: &str) -> IResult<&str, (&str, &str)> {
  let (rest, (_, key, _, _, value, _)) = (
    tag("  "), // 2-space indentation
    take_while1(|c: char| c.is_alphanumeric() || c == '_'),
    char(':'),
    space1,
    take_while1(|c: char| c != '\r' && c != '\n' && c != '#'), // Stop at newline or hash (comments)
    opt(newline), // Make newline optional to handle end-of-file cases
  )
    .parse(input)?;

  Ok((rest, (key, value)))
}

/// Parse a dependencies block and process dependencies without collecting them
/// This uses fold_many0 to avoid Vec allocations
fn parse_dependencies_block(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
  let (rest, (_, _, dependencies)) = (
    tag("  dependencies:"), // 2-space indented dependencies
    newline,
    fold_many0(
      parse_dependency_line,
      Vec::new,
      |mut acc, item| {
        acc.push(item);
        acc
      }
    )
  ).parse(input)?;

  Ok((rest, dependencies))
}

/// Parse a peerDependencies block and process dependencies without collecting them
/// This uses fold_many0 to avoid Vec allocations
fn parse_peer_dependencies_block(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
  let (rest, (_, _, peer_dependencies)) = (
    tag("  peerDependencies:"), // 2-space indented peer dependencies
    newline,
    fold_many0(
      parse_dependency_line,
      Vec::new,
      |mut acc, item| {
        acc.push(item);
        acc
      }
    )
  ).parse(input)?;

  Ok((rest, peer_dependencies))
}

/// Parse a single dependency line with 4-space indentation
/// Example: "    ms: 0.6.2"
fn parse_dependency_line(input: &str) -> IResult<&str, (&str, &str)> {
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

  // Trim whitespace and remove quotes from the range
  let clean_range = dep_range.trim().trim_matches('"');

  Ok((rest, (dep_name, clean_range)))
}

/// Parse a dependency name into an Ident
/// This handles both scoped (@scope/name) and non-scoped (name) packages
fn parse_dependency_name_to_ident(dep_name: &str) -> Ident {
  dep_name.strip_prefix('@').map_or_else(
    || Ident::new(None, dep_name.to_string()),
    |stripped| {
      // Scoped package: @babel/code-frame
      let parts: Vec<&str> = stripped.splitn(2, '/').collect();
      if parts.len() == 2 {
        Ident::new(Some(format!("@{}", parts[0])), parts[1].to_string())
      } else {
        // Malformed scoped package, treat as simple name
        Ident::new(None, dep_name.to_string())
      }
    },
  )
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::HashMap;

  // Helper functions for creating mock data (if needed in the future)
  #[allow(dead_code)]
  fn create_mock_dependencies() -> HashMap<String, String> {
    let mut deps = HashMap::new();
    deps.insert("ms".to_string(), "0.6.2".to_string());
    deps.insert("lodash".to_string(), "^4.17.0".to_string());
    deps
  }

  #[allow(dead_code)]
  fn create_mock_peer_dependencies() -> HashMap<String, String> {
    let mut peer_deps = HashMap::new();
    peer_deps.insert("lodash".to_string(), "^3.0.0 || ^4.0.0".to_string());
    peer_deps
  }

  #[test]
  fn test_parse_dependency_line_simple() {
    let input = "    ms: 0.6.2\n";
    let result = parse_dependency_line(input);

    assert!(result.is_ok(), "Should successfully parse simple dependency line");
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "ms");
    assert_eq!(dep_range, "0.6.2");
  }

  #[test]
  fn test_parse_dependency_line_scoped_package() {
    let input = "    @babel/code-frame: ^7.12.11\n";
    let result = parse_dependency_line(input);

    assert!(result.is_ok(), "Should successfully parse scoped package dependency");
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "@babel/code-frame");
    assert_eq!(dep_range, "^7.12.11");
  }

  #[test]
  fn test_parse_dependency_line_complex_range() {
    let input = "    lodash: ^3.0.0 || ^4.0.0\n";
    let result = parse_dependency_line(input);

    assert!(result.is_ok(), "Should successfully parse complex version range");
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "lodash");
    assert_eq!(dep_range, "^3.0.0 || ^4.0.0");
  }

  #[test]
  fn test_parse_dependencies_block_single_dependency() {
    let input = r#"  dependencies:
    ms: 0.6.2
"#;
    let result = parse_dependencies_block(input);

    assert!(result.is_ok(), "Should successfully parse single dependency block");
    let (remaining, dependencies) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed dependencies
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0], ("ms", "0.6.2"));
  }

  #[test]
  fn test_parse_dependencies_block_multiple_dependencies() {
    let input = r#"  dependencies:
    ms: 0.6.2
    lodash: ^4.17.0
    @babel/core: ^7.12.0
"#;
    let result = parse_dependencies_block(input);

    assert!(result.is_ok(), "Should successfully parse multiple dependencies");
    let (remaining, dependencies) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed dependencies
    assert_eq!(dependencies.len(), 3);
    assert_eq!(dependencies[0], ("ms", "0.6.2"));
    assert_eq!(dependencies[1], ("lodash", "^4.17.0"));
    assert_eq!(dependencies[2], ("@babel/core", "^7.12.0"));
  }

  #[test]
  fn test_parse_dependencies_block_empty() {
    let input = r#"  dependencies:
"#;
    let result = parse_dependencies_block(input);

    assert!(result.is_ok(), "Should successfully parse empty dependencies block");
    let (remaining, dependencies) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dependencies.len(), 0);
  }

  #[test]
  fn test_parse_peer_dependencies_block() {
    let input = r#"  peerDependencies:
    lodash: ^3.0.0 || ^4.0.0
    react: ^16.0.0 || ^17.0.0
"#;
    let result = parse_peer_dependencies_block(input);

    assert!(result.is_ok(), "Should successfully parse peer dependencies block");
    let (remaining, peer_dependencies) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed peer dependencies
    assert_eq!(peer_dependencies.len(), 2);
    assert_eq!(peer_dependencies[0], ("lodash", "^3.0.0 || ^4.0.0"));
    assert_eq!(peer_dependencies[1], ("react", "^16.0.0 || ^17.0.0"));
  }

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

  #[test]
  fn test_parse_descriptor_line_multi_descriptor() {
    let input = r#""c@*, c@workspace:packages/c":"#;
    let result = parse_descriptor_line(input);

    assert!(result.is_ok(), "Should successfully parse multi-descriptor line");
    let (remaining, descriptor) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor (should take the first one: c@*)
    assert_eq!(descriptor.ident().name(), "c");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "*");
  }

  #[test]
  fn test_parse_descriptor_line_complex_multi_descriptor() {
    let input = r#""lodash@npm:^3.0.0 || ^4.0.0, lodash@npm:^4.17.0":"#;
    let result = parse_descriptor_line(input);

    assert!(result.is_ok(), "Should successfully parse complex multi-descriptor line");
    let (remaining, descriptor) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed descriptor (should take the first one)
    assert_eq!(descriptor.ident().name(), "lodash");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(descriptor.range(), "npm:^3.0.0 || ^4.0.0");
  }
}
