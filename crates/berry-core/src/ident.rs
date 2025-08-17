// Types from
// https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-core/sources/types.ts#L19
// TODO - determine if these should be serde[flatten]ed or not

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentName(String);

impl IdentName {
  pub fn new(name: String) -> Self {
    Self(name)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentScope(String);

impl IdentScope {
  pub fn new(scope: String) -> Self {
    Self(scope)
  }

  pub fn as_str(&self) -> &str {
    &self.0
  }
}

/// Scope + name of the package, with hash for comparison
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
  /// The scope of the package, e.g. for `@scope/package`, this is `@scope`
  scope: Option<IdentScope>,
  /// The name of the package, e.g. for `@scope/package`, this is `package`
  name: IdentName,
}

impl Ident {
  pub fn new(scope: Option<String>, name: String) -> Self {
    Self {
      scope: scope.map(IdentScope::new),
      name: IdentName::new(name),
    }
  }

  /// Returns the scope of the package, e.g. for `@scope/package`, this is `@scope`
  pub fn scope(&self) -> Option<&str> {
    self.scope.as_ref().map(IdentScope::as_str)
  }

  /// Returns the name of the package, e.g. for `@scope/package`, this is `package`
  pub fn name(&self) -> &str {
    self.name.as_str()
  }
}

/// The range of a descriptor. Stores the raw string and a precomputed
/// index of the first colon to allow zero-copy access to protocol and selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Range {
  raw: String,
  protocol_sep_index: Option<usize>,
}

impl Range {
  /// Create a Range from a raw string like "npm:^1.2.3" or "*".
  pub fn from_raw(raw: String) -> Self {
    // Find the first ':' to separate protocol and selector
    let protocol_sep_index = raw.find(':');
    Self {
      raw,
      protocol_sep_index,
    }
  }

  /// Returns the raw range string as stored in the lockfile (for round-trip).
  pub fn raw(&self) -> &str {
    &self.raw
  }

  /// Returns the protocol substring if present (e.g., "npm", "workspace", "patch").
  pub fn protocol_str(&self) -> Option<&str> {
    self.protocol_sep_index.map(|i| &self.raw[..i])
  }

  /// Returns the selector part (e.g., "^1.2.3", "packages/a", or the full raw when no protocol).
  pub fn selector(&self) -> &str {
    match self.protocol_sep_index {
      Some(i) => &self.raw[i + 1..],
      None => &self.raw,
    }
  }
}

/// Known protocols supported by Yarn descriptors
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Protocol {
  Npm,
  Workspace,
  Patch,
  Git,
  File,
  Portal,
  Exec,
  Link,
  Unknown,
}

impl Range {
  /// Returns a coarse-grained protocol classification without allocations.
  pub fn protocol(&self) -> Protocol {
    match self.protocol_str() {
      Some("npm") => Protocol::Npm,
      Some("workspace") => Protocol::Workspace,
      Some("patch") => Protocol::Patch,
      Some("file") => Protocol::File,
      Some("portal") => Protocol::Portal,
      Some("exec") => Protocol::Exec,
      Some("link") => Protocol::Link,
      Some(p) if p.starts_with("git") => Protocol::Git,
      Some(_) | None => Protocol::Unknown,
    }
  }

  /// If protocol is npm, returns the semver range selector (e.g., "^1.2.3").
  pub fn as_npm_range(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::Npm => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is workspace, returns the relative workspace path.
  pub fn as_workspace_path(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::Workspace => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is link, returns the path.
  pub fn as_link_path(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::Link => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is file, returns the path.
  pub fn as_file_path(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::File => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is portal, returns the path.
  pub fn as_portal_path(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::Portal => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is exec, returns the command string.
  pub fn as_exec_command(&self) -> Option<&str> {
    match self.protocol() {
      Protocol::Exec => Some(self.selector()),
      _ => None,
    }
  }

  /// If protocol is git, returns (url, optional fragment) where fragment is after '#'.
  pub fn as_git_url_and_fragment(&self) -> Option<(&str, Option<&str>)> {
    match self.protocol() {
      Protocol::Git => {
        // Return full URL including scheme (e.g., git+ssh://...), not just selector
        let raw = self.raw();
        raw.find('#').map_or(Some((raw, None)), |pos| {
          Some((&raw[..pos], Some(&raw[pos + 1..])))
        })
      }
      _ => None,
    }
  }

  /// If protocol is patch, returns (inner, optional source) split at '#'.
  pub fn as_patch_inner_and_source(&self) -> Option<(&str, Option<&str>)> {
    match self.protocol() {
      Protocol::Patch => {
        let sel = self.selector();
        sel.find('#').map_or(Some((sel, None)), |pos| {
          Some((&sel[..pos], Some(&sel[pos + 1..])))
        })
      }
      _ => None,
    }
  }
}

/// Descriptors are just like idents, except that
/// they also contain a range and an additional comparator hash.
///
/// Yarn's `parseRange` to turn a descriptor string into this data structure,
///`makeDescriptor` to create a new one from an ident and a range, or
///`stringifyDescriptor` to generate a string representation of it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Descriptor {
  ident: Ident,
  range: Range,
}

impl Descriptor {
  /// Create a new Descriptor from an Ident and a range string
  pub fn new(ident: Ident, range_raw: String) -> Self {
    Self {
      ident,
      range: Range::from_raw(range_raw),
    }
  }

  /// Returns the Ident of the Descriptor
  pub fn ident(&self) -> &Ident {
    &self.ident
  }

  /// Returns the raw range string of the Descriptor
  pub fn range(&self) -> &str {
    self.range.raw()
  }

  /// Returns the structured range of the Descriptor
  pub fn range_struct(&self) -> &Range {
    &self.range
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_range_no_protocol() {
    let r = Range::from_raw("*".to_string());
    assert_eq!(r.raw(), "*");
    assert_eq!(r.protocol_str(), None);
    assert_eq!(r.selector(), "*");
  }

  #[test]
  fn test_range_with_npm_protocol() {
    let r = Range::from_raw("npm:^2.0.0".to_string());
    assert_eq!(r.raw(), "npm:^2.0.0");
    assert_eq!(r.protocol_str(), Some("npm"));
    assert_eq!(r.selector(), "^2.0.0");
    assert_eq!(r.protocol(), Protocol::Npm);
  }

  #[test]
  fn test_range_with_patch_protocol() {
    let raw = "patch:is-odd@npm%3A3.0.1#~/.yarn/patches/x.patch".to_string();
    let r = Range::from_raw(raw.clone());
    assert_eq!(r.raw(), raw);
    assert_eq!(r.protocol_str(), Some("patch"));
    assert!(r.selector().starts_with("is-odd@npm%3A3.0.1#"));
    assert_eq!(r.protocol(), Protocol::Patch);
  }

  #[test]
  fn test_range_with_link_protocol() {
    let r = Range::from_raw("link:./packages/a".to_string());
    assert_eq!(r.protocol_str(), Some("link"));
    assert_eq!(r.selector(), "./packages/a");
    assert_eq!(r.protocol(), Protocol::Link);
  }

  #[test]
  fn test_range_with_git_protocol() {
    let r = Range::from_raw("git+ssh://host/repo.git#v1".to_string());
    assert_eq!(r.protocol_str(), Some("git+ssh"));
    assert_eq!(r.protocol(), Protocol::Git);
    let (url, frag) = r.as_git_url_and_fragment().unwrap();
    assert_eq!(url, "git+ssh://host/repo.git");
    assert_eq!(frag, Some("v1"));
  }

  #[test]
  fn test_range_protocol_specific_accessors() {
    let npm = Range::from_raw("npm:^1.2.3".to_string());
    assert_eq!(npm.as_npm_range(), Some("^1.2.3"));

    let ws = Range::from_raw("workspace:packages/a".to_string());
    assert_eq!(ws.as_workspace_path(), Some("packages/a"));

    let link = Range::from_raw("link:./local".to_string());
    assert_eq!(link.as_link_path(), Some("./local"));

    let file = Range::from_raw("file:../tarballs/pkg.tgz".to_string());
    assert_eq!(file.as_file_path(), Some("../tarballs/pkg.tgz"));

    let portal = Range::from_raw("portal:./inner".to_string());
    assert_eq!(portal.as_portal_path(), Some("./inner"));

    let exec = Range::from_raw("exec:node ./script.js".to_string());
    assert_eq!(exec.as_exec_command(), Some("node ./script.js"));

    let patch = Range::from_raw(
      "patch:is-odd@npm%3A3.0.1#~/.yarn/patches/is-odd-npm-3.0.1.patch".to_string(),
    );
    let (inner, src) = patch.as_patch_inner_and_source().unwrap();
    assert!(inner.starts_with("is-odd@npm%3A3.0.1"));
    assert!(src.unwrap().starts_with("~/.yarn/patches/"));
  }
}
