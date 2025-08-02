use nom::IResult;
use nom::{
  Parser,
  branch::alt,
  bytes::complete::{is_not, tag, take_until, take_while1},
  character::complete::{char, newline, space0, space1},
  combinator::{map, opt, recognize},
  multi::{fold_many0, many0},
  sequence::{delimited, preceded},
};

use crate::ident::{Descriptor, Ident};
use crate::lockfile::{Lockfile, parse_metadata, parse_yarn_header};
use crate::metadata::{DependencyMeta, PeerDependencyMeta};
use crate::package::{LinkType, Package};

/// Parse just the package from a package entry, discarding the descriptors
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

  Ok((
    rest,
    Lockfile {
      metadata,
      entries: packages,
    },
  ))
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
pub fn parse_package_entry(input: &str) -> IResult<&str, (Vec<Descriptor>, Package)> {
  let (rest, descriptors) = parse_descriptor_line(input)?;
  let (rest, _) = newline.parse(rest)?; // consume newline after descriptor
  let (rest, package) = parse_package_properties(rest)?;

  Ok((rest, (descriptors, package)))
}

/// Parse a package descriptor line like: "debug@npm:1.0.0": or "c@*, c@workspace:packages/c":
pub fn parse_descriptor_line(input: &str) -> IResult<&str, Vec<Descriptor>> {
  let (rest, descriptor_string) =
    delimited(char('"'), take_until("\":"), tag("\":")).parse(input)?;

  // Parse comma-separated descriptors using fold_many0 to avoid allocations
  let (remaining, descriptor_data) = {
    // Parse first descriptor
    let (remaining, first_descriptor) = parse_single_descriptor(descriptor_string)?;

    // Parse subsequent descriptors with separators
    let (remaining, descriptors) = fold_many0(
      preceded((space0, char(','), space0), parse_single_descriptor),
      Vec::new,
      |mut acc, descriptor| {
        acc.push(descriptor);
        acc
      },
    )
    .parse(remaining)?;

    // Combine first descriptor with the rest
    let mut all_descriptors = vec![first_descriptor];
    all_descriptors.extend(descriptors);

    (remaining, all_descriptors)
  };

  // Convert borrowed strings to owned Descriptors (only allocation point)
  let descriptors: Vec<Descriptor> = descriptor_data
    .into_iter()
    .map(|(name_part, protocol, range)| {
      let ident = parse_name_to_ident(name_part);
      let full_range = if protocol.is_empty() {
        range.to_string()
      } else {
        format!("{protocol}:{range}")
      };
      Descriptor::new(ident, full_range)
    })
    .collect();

  assert_eq!(remaining, "", "Should consume entire descriptor string");

  Ok((rest, descriptors))
}

/// Parse a single descriptor string like "debug@npm:1.0.0", "c@*", or "is-odd@patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch"
/// Returns borrowed strings to avoid allocations during parsing
fn parse_single_descriptor(input: &str) -> IResult<&str, (&str, &str, &str)> {
  // Try patch protocol format first (e.g., patch:is-odd@npm%3A3.0.1#~/.yarn/patches/...)
  if let Ok((remaining, (name_part, _, protocol, _, patch_range))) = (
    parse_package_name,
    char('@'),
    parse_protocol,
    char(':'),
    parse_patch_range,
  )
    .parse(input)
  {
    if protocol == "patch" {
      return Ok((remaining, (name_part, protocol, patch_range)));
    }
  }

  // Try protocol:range format (e.g., npm:1.0.0)
  if let Ok((remaining, (name_part, _, protocol, _, range))) = (
    parse_package_name,
    char('@'),
    parse_protocol,
    char(':'),
    take_while1(|c: char| c != ',' && c != '"'),
  )
    .parse(input)
  {
    return Ok((remaining, (name_part, protocol, range)));
  }

  // Try simple range format (e.g., * for c@*)
  if let Ok((remaining, (name_part, _, range))) = (
    parse_package_name,
    char('@'),
    take_while1(|c: char| c != ',' && c != '"'),
  )
    .parse(input)
  {
    return Ok((remaining, (name_part, "", range)));
  }

  Err(nom::Err::Error(nom::error::Error::new(
    input,
    nom::error::ErrorKind::Alt,
  )))
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

/// Parse patch range which can be complex like:
/// - "is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch"
/// - "typescript@npm%3A^5.8.3#optional!builtin<compat/typescript>"
///
/// Returns borrowed string to avoid allocations
fn parse_patch_range(input: &str) -> IResult<&str, &str> {
  take_while1(|c: char| c != ',' && c != '"').parse(input)
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
            package.link_type =
              LinkType::try_from(value).unwrap_or_else(|()| panic!("Invalid link type: {value}"));
          }
          "checksum" => {
            package.checksum = Some(value.to_string());
          }
          "conditions" => {
            package.conditions = Some(value.to_string());
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
          package
            .dependencies
            .insert(descriptor.ident().clone(), descriptor);
        }
      }
      PropertyValue::PeerDependencies(peer_dependencies) => {
        // Store the parsed peer dependencies in the package
        for (dep_name, dep_range) in peer_dependencies {
          let ident = parse_dependency_name_to_ident(dep_name);
          let descriptor = Descriptor::new(ident, dep_range.to_string());
          package
            .peer_dependencies
            .insert(descriptor.ident().clone(), descriptor);
        }
      }
      PropertyValue::Bin(binaries) => {
        // Store the parsed binary executables in the package
        for (bin_name, bin_path) in binaries {
          package
            .bin
            .insert(bin_name.to_string(), bin_path.to_string());
        }
      }
      PropertyValue::DependenciesMeta(meta) => {
        // Store the parsed dependency metadata in the package
        for (dep_name, dep_meta) in meta {
          let ident = parse_dependency_name_to_ident(dep_name);
          package.dependencies_meta.insert(ident, Some(dep_meta));
        }
      }
      PropertyValue::PeerDependenciesMeta(meta) => {
        // Store the parsed peer dependency metadata in the package
        for (dep_name, dep_meta) in meta {
          let ident = parse_dependency_name_to_ident(dep_name);
          package.peer_dependencies_meta.insert(ident, dep_meta);
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

  // Try bin block
  if let Ok((rest, binaries)) = parse_bin_block(input) {
    return Ok((rest, PropertyValue::Bin(binaries)));
  }

  // Try dependenciesMeta block
  if let Ok((rest, meta)) = parse_dependencies_meta_block(input) {
    return Ok((rest, PropertyValue::DependenciesMeta(meta)));
  }

  // Try peerDependenciesMeta block
  if let Ok((rest, meta)) = parse_peer_dependencies_meta_block(input) {
    return Ok((rest, PropertyValue::PeerDependenciesMeta(meta)));
  }

  // If nothing matches, return an error
  Err(nom::Err::Error(nom::error::Error::new(
    input,
    nom::error::ErrorKind::Alt,
  )))
}

/// Enum to represent different types of property values
#[derive(Debug)]
enum PropertyValue<'a> {
  Simple(&'a str, &'a str),
  Dependencies(Vec<(&'a str, &'a str)>), // Use Vec instead of HashMap to avoid allocations
  PeerDependencies(Vec<(&'a str, &'a str)>), // Use Vec instead of HashMap to avoid allocations
  Bin(Vec<(&'a str, &'a str)>),          // Binary executables: name -> path
  DependenciesMeta(Vec<(&'a str, DependencyMeta)>), // Dependency metadata
  PeerDependenciesMeta(Vec<(&'a str, PeerDependencyMeta)>), // Peer dependency metadata
}

/// Parse a simple key-value property line
///
/// # Examples
/// ```
/// let input = r#"  version: 1.0.0"#;
/// let result = parse_simple_property(input);
/// assert!(result.is_ok());
/// let (remaining, (key, value)) = result.unwrap();
/// assert_eq!(remaining, "");
/// assert_eq!(key, "version");
/// assert_eq!(value, "1.0.0");
/// ```
fn parse_simple_property(input: &str) -> IResult<&str, (&str, &str)> {
  let (rest, (_, key, _, _, value, _)) = (
    tag("  "), // 2-space indentation
    take_while1(|c: char| c.is_alphanumeric() || c == '_'),
    char(':'),
    space1,
    is_not("\r\n"), // Stop at newline, don't stop at hash (comments)
    newline,        // Always expect a newline
  )
    .parse(input)?;

  Ok((rest, (key, value)))
}

/// Parse a dependencies block and process dependencies without collecting them
/// This uses `fold_many0` to avoid Vec allocations
fn parse_dependencies_block(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
  let (rest, (_, _, dependencies)) = (
    tag("  dependencies:"), // 2-space indented dependencies
    newline,
    fold_many0(parse_dependency_line, Vec::new, |mut acc, item| {
      acc.push(item);
      acc
    }),
  )
    .parse(input)?;

  Ok((rest, dependencies))
}

/// Parse a peerDependencies block and process dependencies without collecting them
/// This uses `fold_many0` to avoid Vec allocations
fn parse_peer_dependencies_block(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
  let (rest, (_, _, peer_dependencies)) = (
    tag("  peerDependencies:"), // 2-space indented peer dependencies
    newline,
    fold_many0(parse_dependency_line, Vec::new, |mut acc, item| {
      acc.push(item);
      acc
    }),
  )
    .parse(input)?;

  Ok((rest, peer_dependencies))
}

/// Parse a bin block and process binary executables without collecting them
/// This uses `fold_many0` to avoid Vec allocations
fn parse_bin_block(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
  let (rest, (_, _, binaries)) = (
    tag("  bin:"), // 2-space indented bin
    newline,
    fold_many0(parse_bin_line, Vec::new, |mut acc, item| {
      acc.push(item);
      acc
    }),
  )
    .parse(input)?;

  Ok((rest, binaries))
}

/// Parse a dependenciesMeta block and process dependency metadata
fn parse_dependencies_meta_block(input: &str) -> IResult<&str, Vec<(&str, DependencyMeta)>> {
  let (rest, (_, _, meta)) = (
    tag("  dependenciesMeta:"), // 2-space indented dependenciesMeta
    newline,
    fold_many0(parse_dependency_meta_line, Vec::new, |mut acc, item| {
      acc.push(item);
      acc
    }),
  )
    .parse(input)?;

  Ok((rest, meta))
}

/// Parse a peerDependenciesMeta block and process peer dependency metadata
fn parse_peer_dependencies_meta_block(
  input: &str,
) -> IResult<&str, Vec<(&str, PeerDependencyMeta)>> {
  let (rest, (_, _, meta)) = (
    tag("  peerDependenciesMeta:"), // 2-space indented peerDependenciesMeta
    newline,
    fold_many0(
      parse_peer_dependency_meta_line,
      Vec::new,
      |mut acc, item| {
        acc.push(item);
        acc
      },
    ),
  )
    .parse(input)?;

  Ok((rest, meta))
}

/// Parse a single dependency line with 4-space indentation
/// Example: "    ms: 0.6.2" or "    "@actions/io": "npm:^1.0.1""
fn parse_dependency_line(input: &str) -> IResult<&str, (&str, &str)> {
  let (rest, (_, dep_name, _, _, dep_range, _)) = (
    // FIXME: is this part of the spec?
    tag("    "), // 4-space indentation for dependencies
    // Handle both quoted and unquoted dependency names
    alt((
      delimited(
        char('"'),
        take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
        char('"'),
      ),
      take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
    )),
    char(':'),
    space1,
    take_until("\n"), // Take until newline, not just non-newline chars
    newline,
  )
    .parse(input)?;

  // Trim whitespace and remove quotes from the range
  let clean_range = dep_range.trim().trim_matches('"');

  Ok((rest, (dep_name, clean_range)))
}

/// Parse a single bin line with 4-space indentation
/// Example: "    loose-envify: cli.js"
fn parse_bin_line(input: &str) -> IResult<&str, (&str, &str)> {
  let (rest, (_, bin_name, _, _, bin_path, _)) = (
    tag("    "), // 4-space indentation for bin entries
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
    char(':'),
    space1,
    is_not("\r\n"),
    newline,
  )
    .parse(input)?;

  // Trim whitespace and remove quotes from the path
  let clean_path = bin_path.trim().trim_matches('"');

  Ok((rest, (bin_name, clean_path)))
}

/// Parse a single dependency meta line with 4-space indentation
/// Example: "    typescript: { built: true }"
fn parse_dependency_meta_line(input: &str) -> IResult<&str, (&str, DependencyMeta)> {
  let (rest, (_, dep_name, _, _, meta_content, _)) = (
    tag("    "), // 4-space indentation for meta entries
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
    char(':'),
    space1,
    parse_meta_object,
    newline,
  )
    .parse(input)?;

  Ok((rest, (dep_name, meta_content)))
}

/// Parse a single peer dependency meta line with 4-space indentation
/// Example: "    react: { optional: true }"
fn parse_peer_dependency_meta_line(input: &str) -> IResult<&str, (&str, PeerDependencyMeta)> {
  let (rest, (_, dep_name, _, _, meta_content, _)) = (
    tag("    "), // 4-space indentation for meta entries
    take_while1(|c: char| c.is_alphanumeric() || c == '-' || c == '_' || c == '@' || c == '/'),
    char(':'),
    space1,
    parse_peer_meta_object,
    newline,
  )
    .parse(input)?;

  Ok((rest, (dep_name, meta_content)))
}

/// Parse a dependency meta object like "{ built: true, optional: false }"
fn parse_meta_object(input: &str) -> IResult<&str, DependencyMeta> {
  let (rest, _) = char('{')(input)?;
  let (rest, _) = space0(rest)?;

  // Parse properties with optional commas
  let (rest, built) = opt(parse_bool_property("built")).parse(rest)?;
  let (rest, _) = opt((space0, char(','), space0)).parse(rest)?;

  let (rest, optional) = opt(parse_bool_property("optional")).parse(rest)?;
  let (rest, _) = opt((space0, char(','), space0)).parse(rest)?;

  let (rest, unplugged) = opt(parse_bool_property("unplugged")).parse(rest)?;
  let (rest, _) = space0(rest)?;

  let (rest, _) = char('}')(rest)?;

  Ok((
    rest,
    DependencyMeta {
      built,
      optional,
      unplugged,
    },
  ))
}

/// Parse a peer dependency meta object like "{ optional: true }"
fn parse_peer_meta_object(input: &str) -> IResult<&str, PeerDependencyMeta> {
  let (rest, _) = char('{')(input)?;
  let (rest, _) = space0(rest)?;

  let (rest, optional) = parse_bool_property("optional")(rest)?;

  let (rest, _) = char('}')(rest)?;

  Ok((rest, PeerDependencyMeta { optional }))
}

/// Parse a boolean property like "built: true" or "optional: false"
fn parse_bool_property(prop_name: &str) -> impl Fn(&str) -> IResult<&str, bool> {
  move |input| {
    let (rest, (_, _, _, value, _)) = (
      tag(prop_name),
      char(':'),
      space1,
      alt((tag("true"), tag("false"))),
      space0,
    )
      .parse(input)?;

    let bool_value = value == "true";
    Ok((rest, bool_value))
  }
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

    assert!(
      result.is_ok(),
      "Should successfully parse simple dependency line"
    );
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "ms");
    assert_eq!(dep_range, "0.6.2");
  }

  #[test]
  fn test_parse_dependency_line_scoped_package() {
    let input = "    @babel/code-frame: ^7.12.11\n";
    let result = parse_dependency_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse scoped package dependency"
    );
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "@babel/code-frame");
    assert_eq!(dep_range, "^7.12.11");
  }

  #[test]
  fn test_parse_dependency_line_complex_range() {
    let input = "    lodash: ^3.0.0 || ^4.0.0\n";
    let result = parse_dependency_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse complex version range"
    );
    let (remaining, (dep_name, dep_range)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dep_name, "lodash");
    assert_eq!(dep_range, "^3.0.0 || ^4.0.0");
  }

  #[test]
  fn test_parse_dependencies_block_single_dependency() {
    let input = r"  dependencies:
    ms: 0.6.2
";
    let result = parse_dependencies_block(input);

    assert!(
      result.is_ok(),
      "Should successfully parse single dependency block"
    );
    let (remaining, dependencies) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed dependencies
    assert_eq!(dependencies.len(), 1);
    assert_eq!(dependencies[0], ("ms", "0.6.2"));
  }

  #[test]
  fn test_parse_dependencies_block_multiple_dependencies() {
    let input = r"  dependencies:
    ms: 0.6.2
    lodash: ^4.17.0
    @babel/core: ^7.12.0
";
    let result = parse_dependencies_block(input);

    assert!(
      result.is_ok(),
      "Should successfully parse multiple dependencies"
    );
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
    let input = r"  dependencies:
";
    let result = parse_dependencies_block(input);

    assert!(
      result.is_ok(),
      "Should successfully parse empty dependencies block"
    );
    let (remaining, dependencies) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(dependencies.len(), 0);
  }

  #[test]
  fn test_parse_peer_dependencies_block() {
    let input = r"  peerDependencies:
    lodash: ^3.0.0 || ^4.0.0
    react: ^16.0.0 || ^17.0.0
";
    let result = parse_peer_dependencies_block(input);

    assert!(
      result.is_ok(),
      "Should successfully parse peer dependencies block"
    );
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
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
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
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
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
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
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
    let (remaining, (descriptors, package)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
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
    let (remaining, (descriptors, package)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
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

    assert!(
      result.is_ok(),
      "Should successfully parse multi-descriptor line"
    );
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 2);

    // Verify the first descriptor: c@*
    let first_descriptor = &descriptors[0];
    assert_eq!(first_descriptor.ident().name(), "c");
    assert_eq!(first_descriptor.ident().scope(), None);
    assert_eq!(first_descriptor.range(), "*");

    // Verify the second descriptor: c@workspace:packages/c
    let second_descriptor = &descriptors[1];
    assert_eq!(second_descriptor.ident().name(), "c");
    assert_eq!(second_descriptor.ident().scope(), None);
    assert_eq!(second_descriptor.range(), "workspace:packages/c");
  }

  #[test]
  fn test_parse_descriptor_line_complex_multi_descriptor() {
    let input = r#""lodash@npm:^3.0.0 || ^4.0.0, lodash@npm:^4.17.0":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse complex multi-descriptor line"
    );
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 2);

    // Verify the first descriptor
    let first_descriptor = &descriptors[0];
    assert_eq!(first_descriptor.ident().name(), "lodash");
    assert_eq!(first_descriptor.ident().scope(), None);
    assert_eq!(first_descriptor.range(), "npm:^3.0.0 || ^4.0.0");

    // Verify the second descriptor
    let second_descriptor = &descriptors[1];
    assert_eq!(second_descriptor.ident().name(), "lodash");
    assert_eq!(second_descriptor.ident().scope(), None);
    assert_eq!(second_descriptor.range(), "npm:^4.17.0");
  }

  #[test]
  fn test_parse_descriptor_line_patch_protocol() {
    let input =
      r#""is-odd@patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse patch protocol descriptor"
    );
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
    assert_eq!(descriptor.ident().name(), "is-odd");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(
      descriptor.range(),
      "patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch"
    );
  }

  #[test]
  fn test_parse_descriptor_line_builtin_patch() {
    let input =
      r#""typescript@patch:typescript@npm%3A^5.8.3#optional!builtin<compat/typescript>":"#;
    let result = parse_descriptor_line(input);

    assert!(
      result.is_ok(),
      "Should successfully parse builtin patch protocol descriptor"
    );
    let (remaining, descriptors) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
    assert_eq!(descriptor.ident().name(), "typescript");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(
      descriptor.range(),
      "patch:typescript@npm%3A^5.8.3#optional!builtin<compat/typescript>"
    );
  }

  #[test]
  fn test_parse_patch_package_entry() {
    let input = r#""is-odd@patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch":
  version: 3.0.1
  resolution: "is-odd@patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch::version=3.0.1&hash=9b90ad"
  dependencies:
    is-number: "npm:^6.0.0"
  checksum: 4cd944e688e02e147969d6c1784bad1156f6084edbbd4d688f6a37b5fc764671aa99679494fc0bfaf623919bea2779e724fffc31c6ee0432b7c91f174526e5fe
  languageName: node
  linkType: hard

"#;
    let result = parse_package_entry(input);

    assert!(
      result.is_ok(),
      "Should successfully parse patch package entry"
    );
    let (remaining, (descriptors, package)) = result.unwrap();
    assert_eq!(remaining, "");
    assert_eq!(descriptors.len(), 1);

    // Verify the parsed descriptor
    let descriptor = &descriptors[0];
    assert_eq!(descriptor.ident().name(), "is-odd");
    assert_eq!(descriptor.ident().scope(), None);
    assert_eq!(
      descriptor.range(),
      "patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch"
    );

    // Verify the parsed package
    assert_eq!(package.version, Some("3.0.1".to_string()));
    assert_eq!(
      package.resolution,
      Some("is-odd@patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1-93c3c3f41b.patch::version=3.0.1&hash=9b90ad".to_string())
    );
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
    assert_eq!(package.checksum, Some("4cd944e688e02e147969d6c1784bad1156f6084edbbd4d688f6a37b5fc764671aa99679494fc0bfaf623919bea2779e724fffc31c6ee0432b7c91f174526e5fe".to_string()));
  }

  #[test]
  fn test_parse_package_properties_with_bin() {
    let input = r#"  version: 1.4.0
  resolution: "loose-envify@npm:1.4.0"
  dependencies:
    js-tokens: "npm:^3.0.0 || ^4.0.0"
  bin:
    loose-envify: cli.js
  checksum: 10/6517e24e0cad87ec9888f500c5b5947032cdfe6ef65e1c1936a0c48a524b81e65542c9c3edc91c97d5bddc806ee2a985dbc79be89215d613b1de5db6d1cfe6f4
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with bin field"
    );
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed package properties
    assert_eq!(package.version, Some("1.4.0".to_string()));
    assert_eq!(
      package.resolution,
      Some("loose-envify@npm:1.4.0".to_string())
    );
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
    assert_eq!(package.checksum, Some("10/6517e24e0cad87ec9888f500c5b5947032cdfe6ef65e1c1936a0c48a524b81e65542c9c3edc91c97d5bddc806ee2a985dbc79be89215d613b1de5db6d1cfe6f4".to_string()));

    // Verify the bin field is correctly stored
    assert_eq!(package.bin.len(), 1);
    assert_eq!(package.bin.get("loose-envify"), Some(&"cli.js".to_string()));
  }

  #[test]
  fn test_parse_package_properties_with_conditions() {
    let input = r#"  version: 1.4.0
  resolution: "loose-envify@npm:1.4.0"
  conditions: os=linux & cpu=x64 & libc=glibc
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with conditions field"
    );
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the parsed package properties
    assert_eq!(package.version, Some("1.4.0".to_string()));
    assert_eq!(
      package.resolution,
      Some("loose-envify@npm:1.4.0".to_string())
    );
    assert_eq!(
      package.conditions,
      Some("os=linux & cpu=x64 & libc=glibc".to_string())
    );
    assert_eq!(package.language_name.as_ref(), "node");
    assert_eq!(package.link_type, LinkType::Hard);
  }

  #[test]
  fn test_parse_package_properties_with_multiple_bin() {
    let input = r#"  version: 1.0.0
  resolution: "test-package@npm:1.0.0"
  bin:
    test-cli: bin/cli.js
    test-server: bin/server.js
    test-utils: bin/utils.js
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with multiple bin entries"
    );
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the bin field is correctly stored
    assert_eq!(package.bin.len(), 3);
    assert_eq!(package.bin.get("test-cli"), Some(&"bin/cli.js".to_string()));
    assert_eq!(
      package.bin.get("test-server"),
      Some(&"bin/server.js".to_string())
    );
    assert_eq!(
      package.bin.get("test-utils"),
      Some(&"bin/utils.js".to_string())
    );
  }

  #[test]
  fn test_parse_package_properties_with_dependencies_meta() {
    let input = r#"  version: 1.0.0
  resolution: "test-package@npm:1.0.0"
  dependenciesMeta:
    typescript: { built: true, optional: false }
    react: { built: false, optional: true, unplugged: true }
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with dependenciesMeta"
    );
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the dependenciesMeta field is correctly stored
    assert_eq!(package.dependencies_meta.len(), 2);

    let typescript_meta = package
      .dependencies_meta
      .get(&Ident::new(None, "typescript".to_string()))
      .unwrap()
      .as_ref()
      .unwrap();
    assert_eq!(typescript_meta.built, Some(true));
    assert_eq!(typescript_meta.optional, Some(false));
    assert_eq!(typescript_meta.unplugged, None);

    let react_meta = package
      .dependencies_meta
      .get(&Ident::new(None, "react".to_string()))
      .unwrap()
      .as_ref()
      .unwrap();
    assert_eq!(react_meta.built, Some(false));
    assert_eq!(react_meta.optional, Some(true));
    assert_eq!(react_meta.unplugged, Some(true));
  }

  #[test]
  fn test_parse_package_properties_with_peer_dependencies_meta() {
    let input = r#"  version: 1.0.0
  resolution: "test-package@npm:1.0.0"
  peerDependenciesMeta:
    react: { optional: true }
    vue: { optional: false }
  languageName: node
  linkType: hard
"#;
    let result = parse_package_properties(input);

    assert!(
      result.is_ok(),
      "Should successfully parse package properties with peerDependenciesMeta"
    );
    let (remaining, package) = result.unwrap();
    assert_eq!(remaining, "");

    // Verify the peerDependenciesMeta field is correctly stored
    assert_eq!(package.peer_dependencies_meta.len(), 2);

    let react_meta = package
      .peer_dependencies_meta
      .get(&Ident::new(None, "react".to_string()))
      .unwrap();
    assert!(react_meta.optional);

    let vue_meta: &PeerDependencyMeta = package
      .peer_dependencies_meta
      .get(&Ident::new(None, "vue".to_string()))
      .unwrap();
    assert!(vue_meta.optional);
  }

  #[test]
  fn test_parse_multiple_packages() {
    // Test parsing multiple packages with a simple example
    let input = r#"# This file is generated by running "yarn install" inside your project.
# Manual changes might be lost - proceed with caution!

__metadata:
  version: 8
  cacheKey: 10

"@actions/http-client@npm:^2.2.0":
  version: 2.2.3
  resolution: "@actions/http-client@npm:2.2.3"
  languageName: node
  linkType: hard

"@actions/io@npm:^1.0.1, @actions/io@npm:^1.1.3":
  version: 1.1.3
  resolution: "@actions/io@npm:1.1.3"
  languageName: node
  linkType: hard
"#;

    let result = parse_lockfile(input);

    match result {
      Ok((remaining, lockfile)) => {
        println!("Successfully parsed {} packages", lockfile.entries.len());
        // print first 100 chars of remaining
        println!(
          "First 100 chars of remaining: '{}'",
          &remaining[..100.min(remaining.len())]
        );
        assert_eq!(lockfile.entries.len(), 2, "Should parse 2 packages");
        assert!(remaining.is_empty(), "Should consume all input");
      }
      Err(e) => {
        println!("Parse error: {e:?}");
        panic!("Failed to parse multiple packages");
      }
    }
  }
}
