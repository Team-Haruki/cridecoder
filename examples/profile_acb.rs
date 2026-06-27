//! Micro-profile of ACB extraction stages on a real ACB.
//! Usage: cargo run --release --example profile_acb -- <path-to.acb>

use std::io::Cursor;
use std::time::Instant;

use cridecoder::acb::{AfsArchive, TrackList, UtfTable};

fn time<T>(label: &str, rounds: u32, mut f: impl FnMut() -> T) -> T {
    for _ in 0..3 {
        std::hint::black_box(f());
    }
    let mut best = f64::INFINITY;
    let mut last = None;
    for _ in 0..rounds {
        let t0 = Instant::now();
        let r = std::hint::black_box(f());
        best = best.min(t0.elapsed().as_secs_f64());
        last = Some(r);
    }
    println!("  {label:40} best={:8.4} ms", best * 1000.0);
    last.unwrap()
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: profile_acb <acb>");
    let data = std::fs::read(&path).unwrap();
    println!("ACB: {} ({} bytes)\n", path, data.len());
    let rounds = 300;

    time("UtfTable::new (outer)", rounds, || {
        UtfTable::new(Cursor::new(&data)).unwrap()
    });

    time("UtfTable::new + TrackList::new", rounds, || {
        let utf = UtfTable::new(Cursor::new(&data)).unwrap();
        TrackList::new(&utf).unwrap()
    });

    time("+ build embedded AFS2", rounds, || {
        let utf = UtfTable::new(Cursor::new(&data)).unwrap();
        let _tl = TrackList::new(&utf).unwrap();
        let awb = utf.rows[0]
            .get("AwbFile")
            .unwrap()
            .as_bytes()
            .unwrap()
            .to_vec();
        AfsArchive::new(Cursor::new(awb)).unwrap()
    });

    time("extract_acb_to_memory (full)", rounds, || {
        cridecoder::extract_acb_to_memory(Cursor::new(&data), None).unwrap()
    });
}
