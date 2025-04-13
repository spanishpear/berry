/// The start of the metadata block
/// Typically at the start of the file
pub struct Metadata {
  version: String,
  cache_key: String,
}

// https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-core/sources/Manifest.ts#L25
// note: this smells like option, but realistically it is an extra property
#[derive(Debug)]
pub struct PeerDependencyMeta {
  optional: bool,
}

// https://github.com/yarnpkg/berry/blob/master/packages/yarnpkg-core/sources/Manifest.ts#L19
// note: this smells like misuse of option, but realistically it is an extra property that
// may exist, and it may be true/false
#[derive(Debug)]
pub struct DependencyMeta {
  built: Option<bool>,
  optional: Option<bool>,
  unplugged: Option<bool>,
}
