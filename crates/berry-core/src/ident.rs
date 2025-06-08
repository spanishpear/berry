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

  pub fn scope(&self) -> Option<&str> {
    self.scope.as_ref().map(IdentScope::as_str)
  }

  pub fn name(&self) -> &str {
    self.name.as_str()
  }
}

/// The range of the Descriptor, e.g. `^1.2.3`, `~1.2.3`, `1.2.x`, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentRange(String);

impl IdentRange {
  pub fn new(range: String) -> Self {
    Self(range)
  }

  pub fn as_str(&self) -> &str {
    &self.0
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
  range: IdentRange,
}

impl Descriptor {
  pub fn new(ident: Ident, range: String) -> Self {
    Self {
      ident,
      range: IdentRange::new(range),
    }
  }

  pub fn ident(&self) -> &Ident {
    &self.ident
  }

  pub fn range(&self) -> &str {
    self.range.as_str()
  }
}
