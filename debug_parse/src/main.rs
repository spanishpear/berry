use berry_core::parse::parse_lockfile;
use berry_test::load_fixture;

fn main() {
    let fixture = load_fixture("resolutions-patches.yarn.lock");
    println!("File size: {} bytes", fixture.len());
    println!("File lines: {}", fixture.lines().count());

    // Show first 500 characters
    println!("First 500 chars:");
    println!("{}", &fixture[..fixture.len().min(500)]);

    let result = parse_lockfile(&fixture);
    match result {
        Ok((rest, lockfile)) => {
            println!("Parse successful!");
            println!("Remaining unparsed: {} bytes", rest.len());
            println!("Packages parsed: {}", lockfile.entries.len());
            println!("Metadata version: {}", lockfile.metadata.version);

            if !rest.is_empty() {
                println!("First 500 chars of unparsed content:");
                println!("{}", &rest[..rest.len().min(500)]);

                // Find the first package entry in the unparsed content
                if let Some(pos) = rest.find("\"@") {
                    println!("First package entry in unparsed content (around position {}):", pos);
                    let start = pos.saturating_sub(50);
                    let end = (pos + 200).min(rest.len());
                    println!("{}", &rest[start..end]);
                }

                // Test parsing the problematic section
                println!("\nTesting problematic section:");
                let problematic_section = "  peerDependenciesMeta:\n    graphql-ws:\n      optional: true\n    react:\n      optional: true\n    react-dom:\n      optional: true\n    subscriptions-transport-ws:\n      optional: true\n";
                println!("Problematic section: {}", problematic_section);

                // Try to parse this section manually
                use berry_core::parse::parse_peer_dependencies_meta_block;
                match parse_peer_dependencies_meta_block(problematic_section) {
                    Ok((rest, meta)) => {
                        println!("Successfully parsed peerDependenciesMeta: {:?}", meta);
                        println!("Remaining: '{}'", rest);
                    }
                    Err(e) => {
                        println!("Failed to parse peerDependenciesMeta: {:?}", e);
                    }
                }
            }
        }
        Err(e) => {
            println!("Parse failed: {:?}", e);
        }
    }
}
