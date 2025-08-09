use berry_core::parse::parse_lockfile;
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "berry-dump")]
#[command(about = "Parse a Yarn Berry lockfile and dump the Rust struct")]
struct Args {
  /// Path to a lockfile to parse
  #[arg(value_name = "LOCKFILE")]
  lockfile: Option<PathBuf>,

  /// Use a bundled fixture name instead of a path (reads from repo fixtures/)
  #[arg(short, long, value_name = "NAME")]
  fixture: Option<String>,
}

fn read_file(p: &PathBuf) -> String {
  std::fs::read_to_string(p).expect("failed to read file")
}

fn main() {
  let args = Args::parse();

  let contents = if let Some(fixture) = &args.fixture {
    let fixtures_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
      .parent()
      .unwrap()
      .parent()
      .unwrap()
      .join("fixtures");
    let path = fixtures_dir.join(fixture);
    read_file(&path)
  } else if let Some(path) = &args.lockfile {
    read_file(path)
  } else {
    eprintln!("Provide a lockfile path or --fixture <name>");
    std::process::exit(2);
  };

  match parse_lockfile(&contents) {
    Ok((remaining, lockfile)) => {
      println!("lockfile: {:#?}", lockfile);
      if !remaining.trim().is_empty() {
        eprintln!("WARNING: {} bytes remaining unparsed", remaining.len());
      }
    }
    Err(e) => {
      eprintln!("Parse error: {e:?}");
      std::process::exit(1);
    }
  }
}
