//! Test USM extraction

use cridecoder::extract_usm_file;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: test_usm <usm_file>");
        std::process::exit(1);
    }

    let usm_file = Path::new(&args[1]);
    let output_dir = Path::new("test_output_usm");
    println!("Testing USM extraction: {:?}", usm_file);

    // Use video key None for testing (no encryption)
    match extract_usm_file(usm_file, output_dir, None, false) {
        Ok(files) => {
            println!("Successfully extracted {} files", files.len());
            for file in &files {
                println!("  {:?}", file);
            }
        }
        Err(e) => {
            eprintln!("Error extracting USM: {:?}", e);
            std::process::exit(1);
        }
    }
}
