//! Test HCA to WAV conversion

use cridecoder::HcaDecoder;
use std::env;
use std::fs::File;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: test_hca <hca_file> <output_wav>");
        std::process::exit(1);
    }

    let hca_file = &args[1];
    let wav_file = &args[2];
    println!("Testing HCA decode: {} -> {}", hca_file, wav_file);

    let mut decoder = match HcaDecoder::from_file(hca_file) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Error opening HCA: {:?}", e);
            std::process::exit(1);
        }
    };

    let info = decoder.info();
    println!("  Sample rate: {}", info.sampling_rate);
    println!("  Channels: {}", info.channel_count);
    println!("  Block count: {}", info.block_count);
    println!("  Block size: {}", info.block_size);
    println!("  Samples per block: {}", info.samples_per_block);
    println!("  Encoder delay: {}", info.encoder_delay);

    let mut out = File::create(wav_file).expect("Failed to create output file");
    match decoder.decode_to_wav(&mut out) {
        Ok(()) => {
            let meta = std::fs::metadata(wav_file).unwrap();
            println!("Success! Output: {} ({} bytes)", wav_file, meta.len());
        }
        Err(e) => {
            eprintln!("Error decoding HCA: {:?}", e);
            std::process::exit(1);
        }
    }
}
