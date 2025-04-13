use crate::ident::Ident;

/// Locators are just like idents (including their `identHash`), except that
/// they also contain a reference and an additional comparator hash. They are
/// in this regard very similar to descriptors except that each descriptor may
/// reference multiple valid candidate packages whereas each locators can only
/// reference a single package.
///
/// This interesting property means that each locator can be safely turned into
/// a descriptor - but not the other way
/// around (except in very specific cases).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locator {
  ident: Ident,
  /// A package reference uniquely identifies a package (eg. `1.2.3`).
  reference: String,
}
