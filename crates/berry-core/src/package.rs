use crate::ident::{Descriptor, Ident};
use crate::metadata::{DependencyMeta, PeerDependencyMeta};
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
/// The type of link to use for a package
pub enum LinkType {
  /// The package manager owns the location (typically things within the cache)
  /// e.g. `PnP` linker may unplug packages
  Hard,

  /// The package manager doesn't own the location (symlinks, workspaces, etc),
  /// so the linkers aren't allowed to do anything with them except use them as
  /// they are.
  Soft,
}

// is there a derive for this?
impl TryFrom<&str> for LinkType {
  type Error = ();

  fn try_from(s: &str) -> Result<Self, Self::Error> {
    match s {
      "hard" => Ok(Self::Hard),
      "soft" => Ok(Self::Soft),
      _ => Err(()),
    }
  }
}

/// The name of the binary being shipped by a dependency
/// e.g. `napi`, `taplo`, `yarn`
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[allow(dead_code)]
struct BinaryName(String);

/// <https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-fslib/sources/path.ts#L9>
/// note - yarn uses internal types to differ between file paths and portable paths
/// The path to the binary being shipped by a dependency
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
struct PortablePath(String);

/// The resolved(?) version of the package dependency
/// e.g. `1.2.3`, `1.2.3-beta.1`, `0.0.0-use-local`
/// note: Not an identifier, as this is a literal version
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
struct PackageVersion(String);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageName(String);

impl LanguageName {
  pub fn new(name: String) -> Self {
    Self(name)
  }
}

impl AsRef<str> for LanguageName {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

// TODO: should the strings here be owned, or just &str for 'a
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)]
pub struct Package {
  /// Version of the package, if available
  pub version: Option<String>,

  /// Resolution string for the package
  pub resolution: Option<String>,

  /// The "language" of the package (eg. `node`), for use with multi-linkers.
  pub language_name: LanguageName,

  /// Type of filesystem link for a pacakge
  pub link_type: LinkType,

  /// Checksum for the package
  pub checksum: Option<String>,

  /// A set of constraints indicating whether the package supports the host environments
  conditions: Option<String>,

  /// A map of the package's dependencies. There's no distinction between prod
  /// dependencies and dev dependencies, because those have already been merged
  /// during the resolution process
  pub dependencies: HashMap<Ident, Descriptor>,

  /// Map with additional information about direct dependencies
  dependencies_meta: HashMap<Ident, Option<DependencyMeta>>,

  /// Map of pacakges peer dependencies
  pub peer_dependencies: HashMap<Ident, Descriptor>,

  /// Map with additional information about peer dependencies
  peer_dependencies_meta: HashMap<Ident, PeerDependencyMeta>,

  /// all bin entries for the package
  ///
  /// We don't need binaries in resolution, but we do neeed them to keep `yarn run` fast
  /// else we have to parse and read all of the zipfiles
  bin: HashMap<BinaryName, PortablePath>,
}

impl Package {
  pub fn new(language_name: String, link_type: LinkType) -> Self {
    Self {
      version: None,
      resolution: None,
      language_name: LanguageName::new(language_name),
      link_type,
      checksum: None,
      conditions: None,
      dependencies: HashMap::new(),
      dependencies_meta: HashMap::new(),
      peer_dependencies: HashMap::new(),
      peer_dependencies_meta: HashMap::new(),
      bin: HashMap::new(),
    }
  }

  #[must_use]
  pub fn with_version(mut self, version: String) -> Self {
    self.version = Some(version);
    self
  }

  #[must_use]
  pub fn with_resolution(mut self, resolution: String) -> Self {
    self.resolution = Some(resolution);
    self
  }

  #[must_use]
  pub fn with_checksum(mut self, checksum: String) -> Self {
    self.checksum = Some(checksum);
    self
  }
}

pub type LockfileEntry = Package;
