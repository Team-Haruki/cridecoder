//! Test ACB extraction

use cridecoder::extract_acb_from_file;
use std::env;
use std::path::Path;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: test_acb <acb_file>");
        std::process::exit(1);
    }

    let acb_file = Path::new(&args[1]);
    let output_dir = Path::new("test_output_acb");
    println!("Testing ACB extraction: {:?}", acb_file);
    
    match extract_acb_from_file(acb_file, output_dir) {
        Ok(Some(tracks)) => {
            println!("Successfully extracted {} tracks", tracks.len());
            for (i, track) in tracks.iter().enumerate() {
                println!("  Track {}: {}", i, track);
            }
        }
        Ok(None) => {
            println!("No tracks found");
        }
        Err(e) => {
            eprintln!("Error extracting ACB: {:?}", e);
            std::process::exit(1);
        }
    }
}
