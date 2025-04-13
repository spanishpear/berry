use crate::{metadata::Metadata, package::LockfileEntry};

/// A serialized representation of a yarn lockfile.
pub struct Lockfile {
  /// Lockfile version and cache key
  pub metadata: Metadata,
  /// The entries in the lockfile
  pub entries: Vec<LockfileEntry>,
}
