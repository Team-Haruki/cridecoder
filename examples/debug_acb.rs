//! Debug ACB extraction

use std::env;
use std::fs::File;
use std::io::Read;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: debug_acb <acb_file>");
        std::process::exit(1);
    }

    let acb_file = &args[1];
    println!("Debugging ACB: {}", acb_file);
    
    let mut file = File::open(acb_file).expect("Failed to open file");
    let mut data = Vec::new();
    file.read_to_end(&mut data).expect("Failed to read file");
    
    println!("File size: {} bytes", data.len());
    println!("First 64 bytes: {:02x?}", &data[..64.min(data.len())]);
    
    // Check for @UTF signature
    if data.len() >= 4 && &data[0..4] == b"@UTF" {
        println!("Found @UTF signature at offset 0");
    }
    
    // Search for AFS2 signature
    for i in 0..data.len().saturating_sub(4) {
        if &data[i..i+4] == b"AFS2" {
            println!("Found AFS2 signature at offset 0x{:x}", i);
        }
    }
    
    // Search for HCA signature
    for i in 0..data.len().saturating_sub(4) {
        if &data[i..i+4] == b"HCA\x00" || (data[i] == 0x80 && data[i+1] == 0x00) {
            println!("Possible HCA at offset 0x{:x}", i);
            break;
        }
    }
}
