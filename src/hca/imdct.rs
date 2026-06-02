//! IMDCT (Inverse Modified Discrete Cosine Transform) for HCA decoding

use super::decoder::{StChannel, HCA_SAMPLES_PER_SUBFRAME};
use super::tables::IMDCT_WINDOW;
use rustdct::{Dct4, DctPlanner};
use std::sync::{Arc, LazyLock};

const HALF: usize = HCA_SAMPLES_PER_SUBFRAME / 2;
const HCA_DCT4_SCALE: f32 = 0.125;

static DCT4_128: LazyLock<Arc<dyn Dct4<f32>>> = LazyLock::new(|| {
    let mut planner = DctPlanner::new();
    planner.plan_dct4(HCA_SAMPLES_PER_SUBFRAME)
});

/// Apply IMDCT transform to dequantized spectra
pub fn imdct_transform(ch: &mut StChannel, subframe: usize) {
    let size = HCA_SAMPLES_PER_SUBFRAME;
    let dct = &*DCT4_128;
    let spectra = &mut ch.spectra[subframe];

    // HCA's previous hand-written butterfly/DCT-IV stage is equivalent to
    // rustdct's unnormalized DCT-IV output scaled by 1/8.
    dct.process_dct4_with_scratch(spectra, &mut ch.temp);
    for sample in spectra.iter_mut() {
        *sample *= HCA_DCT4_SCALE;
    }

    for i in 0..HALF {
        ch.wave[subframe][i] = IMDCT_WINDOW[i] * spectra[i + HALF] + ch.imdct_previous[i];
        ch.wave[subframe][i + HALF] =
            IMDCT_WINDOW[i + HALF] * spectra[size - 1 - i] - ch.imdct_previous[i + HALF];
        ch.imdct_previous[i] = IMDCT_WINDOW[size - 1 - i] * spectra[HALF - i - 1];
        ch.imdct_previous[i + HALF] = IMDCT_WINDOW[HALF - i - 1] * spectra[i];
    }
}
