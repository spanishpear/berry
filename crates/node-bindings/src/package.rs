use crate::ident::{Descriptor, Ident};
use crate::metadata::{DependencyMeta, PeerDependencyMeta};
use std::collections::HashMap;

#[derive(Debug)]
/// The type of link to use for a package
pub enum LinkType {
  /// The package manager owns the location (typically things within the cache)
  /// e.g. PnP linker may unplug packages
  Hard,

  /// The package manager doesn't own the location (symlinks, workspaces, etc),
  /// so the linkers aren't allowed to do anything with them except use them as
  /// they are.
  Soft,
}

/// The name of the binary being shipped by a dependency
/// e.g. `napi`, `taplo`, `yarn`
#[derive(Debug)]
struct BinaryName(String);

/// https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-fslib/sources/path.ts#L9
/// note - yarn uses internal types to differ between file paths and portable paths
/// The path to the binary being shipped by a dependency
#[derive(Debug)]
struct PortablePath(String);

/// The resolved(?) version of the package dependency
/// e.g. `1.2.3`, `1.2.3-beta.1`, `0.0.0-use-local`
/// note: Not an identifier, as this is a literal version
#[derive(Debug)]
struct PackageVersion(String);

#[derive(Debug)]
struct LanguageName(String);

// TODO: should the strings here be owned, or just &str for 'a
#[derive(Debug)]
pub struct Package {
  /// Version of the package, if available
  version: Option<String>,

  /// The "language" of the package (eg. `node`), for use with multi-linkers.
  language_name: LanguageName,

  /// Type of filesystem link for a pacakge
  link_type: LinkType,

  /// A set of constraints indicating whether the package supports the host environments
  conditions: Option<String>,

  /// A map of the package's dependencies. There's no distinction between prod
  /// dependencies and dev dependencies, because those have already been merged
  /// during the resolution process
  dependencies: HashMap<Ident, Descriptor>,

  /// Map with additional information about direct dependencies
  dependencies_meta: HashMap<Ident, Option<DependencyMeta>>,

  /// Map of pacakges peer dependencies
  peer_dependencies: HashMap<Ident, Descriptor>,

  /// Map with additional information about peer dependencies
  peer_dependencies_meta: HashMap<Ident, PeerDependencyMeta>,

  /// all bin entries for the package
  ///
  /// We don't need binaries in resolution, but we do neeed them to keep `yarn run` fast
  /// else we have to parse and read all of the zipfiles
  bin: HashMap<BinaryName, PortablePath>,
}

pub type LockfileEntry = Package;
