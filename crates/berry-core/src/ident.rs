// Types from
// https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-core/sources/types.ts#L19
// TODO - determine if these should be serde[flatten]ed or not

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentName(String);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentScope(String);

/// Scope + name of the package, with hash for comparison
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Ident {
  /// The scope of the package, e.g. for `@scope/package`, this is `@scope`
  scope: Option<IdentScope>,
  /// The name of the package, e.g. for `@scope/package`, this is `package`
  name: IdentName,
}

/// The range of the Descriptor, e.g. `^1.2.3`, `~1.2.3`, `1.2.x`, etc.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct IdentRange(String);

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
