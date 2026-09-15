//! A stereo-linked compressor for one source.
//!
//! The louder channel drives one shared detector, so a sound panned to one
//! side cannot pull the stereo image toward the other. The detector envelope
//! and gain survive block boundaries and settings-only rebuilds.

use super::super::mixer::{CHANNELS, SAMPLE_RATE};

const MIN_LEVEL: f32 = 1.0e-9;

#[derive(Debug, Clone, PartialEq)]
pub struct Compressor {
    spec: CompressorSpec,
    envelope: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompressorSpec {
    threshold: f32,
    ratio: f32,
    attack_coefficient: f32,
    release_coefficient: f32,
    output_gain: f32,
}

impl CompressorSpec {
    pub fn new(
        threshold_percent: u32,
        ratio: u32,
        attack_ms: u32,
        release_ms: u32,
        output_gain_percent: u32,
    ) -> Self {
        Self {
            // Zero is not useful for a logarithmic threshold and would make
            // every non-silent sample compress, so corrupted settings clamp
            // to the smallest value the UI offers.
            threshold: threshold_percent.clamp(1, 100) as f32 / 100.0,
            ratio: ratio.clamp(1, 20) as f32,
            attack_coefficient: time_coefficient(attack_ms),
            release_coefficient: time_coefficient(release_ms),
            output_gain: output_gain_percent.min(500) as f32 / 100.0,
        }
    }
}

fn time_coefficient(ms: u32) -> f32 {
    if ms == 0 {
        0.0
    } else {
        (-1.0 / (ms as f32 * 0.001 * SAMPLE_RATE as f32)).exp()
    }
}

impl Compressor {
    pub fn new(spec: CompressorSpec) -> Self {
        Self {
            spec,
            envelope: 0.0,
        }
    }

    pub fn adopt(&mut self, spec: CompressorSpec) {
        self.spec = spec;
    }

    pub fn process(&mut self, block: &mut [f32]) {
        for frame in block.chunks_mut(CHANNELS) {
            let detected = frame.iter().map(|sample| sample.abs()).fold(0.0, f32::max);
            let coefficient = if detected > self.envelope {
                self.spec.attack_coefficient
            } else {
                self.spec.release_coefficient
            };
            self.envelope = coefficient * self.envelope + (1.0 - coefficient) * detected;

            let reduction = if self.envelope > self.spec.threshold {
                // Work in decibels so ratio has its conventional meaning: at
                // 4:1, an input 12 dB over threshold leaves 3 dB over it.
                let above_db = 20.0 * (self.envelope / self.spec.threshold).log10();
                10.0f32.powf(-(above_db * (1.0 - 1.0 / self.spec.ratio)) / 20.0)
            } else {
                1.0
            };
            let gain = reduction * self.spec.output_gain;
            for sample in frame {
                *sample *= gain;
                if !sample.is_finite() {
                    *sample = 0.0;
                }
            }
            if !self.envelope.is_finite() {
                self.envelope = MIN_LEVEL;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mixer::BLOCK_SAMPLES;

    fn settled_block(compressor: &mut Compressor, level: f32, blocks: usize) -> Vec<f32> {
        let mut result = Vec::new();
        for _ in 0..blocks {
            result = vec![level; BLOCK_SAMPLES];
            compressor.process(&mut result);
        }
        result
    }

    #[test]
    fn audio_below_threshold_passes_unchanged() {
        let mut compressor = Compressor::new(CompressorSpec::new(20, 4, 0, 100, 100));
        let block = settled_block(&mut compressor, 0.1, 2);
        assert!(block.iter().all(|sample| (*sample - 0.1).abs() < 1.0e-6));
    }

    #[test]
    fn ratio_reduces_level_above_threshold() {
        let mut compressor = Compressor::new(CompressorSpec::new(25, 4, 0, 100, 100));
        let block = settled_block(&mut compressor, 1.0, 2);
        // Full scale is 12 dB above 25%; 4:1 leaves it 3 dB above, or ~0.354.
        assert!((block[BLOCK_SAMPLES - 1] - 0.354).abs() < 0.002);
    }

    #[test]
    fn output_gain_is_applied_after_compression() {
        let mut compressor = Compressor::new(CompressorSpec::new(100, 4, 0, 100, 150));
        let block = settled_block(&mut compressor, 0.2, 1);
        assert!((block[0] - 0.3).abs() < 1.0e-6);
    }

    #[test]
    fn stereo_channels_receive_the_same_gain() {
        let mut compressor = Compressor::new(CompressorSpec::new(25, 4, 0, 100, 100));
        let mut block = vec![0.0; BLOCK_SAMPLES];
        for frame in block.chunks_mut(CHANNELS) {
            frame[0] = 1.0;
            frame[1] = 0.5;
        }
        compressor.process(&mut block);
        let last = &block[block.len() - 2..];
        assert!((last[0] / last[1] - 2.0).abs() < 1.0e-6);
    }

    #[test]
    fn adopting_settings_keeps_the_detector_envelope() {
        let mut compressor = Compressor::new(CompressorSpec::new(20, 4, 0, 100, 100));
        settled_block(&mut compressor, 1.0, 2);
        let envelope = compressor.envelope;
        compressor.adopt(CompressorSpec::new(30, 3, 20, 200, 120));
        assert_eq!(compressor.envelope, envelope);
    }
}
