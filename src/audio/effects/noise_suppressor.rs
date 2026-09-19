//! RNNoise background-noise suppression for one stereo source.
//!
//! RNNoise processes one 480-sample mono frame at 48 kHz. The mixer supplies
//! exactly that duration every block, so each stereo channel has its own state
//! and no resampler or additional queue is needed. This has the model's normal
//! one-frame algorithmic lookback, but never adds scheduling latency.

use super::super::mixer::{BLOCK_FRAMES, CHANNELS};
use nnnoiseless::DenoiseState;
use std::fmt;

const PCM_SCALE: f32 = i16::MAX as f32;

/// A running RNNoise processor, one state per channel to retain stereo audio.
#[derive(Clone)]
pub struct NoiseSuppressor {
    spec: NoiseSuppressorSpec,
    states: [Box<DenoiseState<'static>>; CHANNELS],
    input: [[f32; BLOCK_FRAMES]; CHANNELS],
    output: [[f32; BLOCK_FRAMES]; CHANNELS],
}

impl fmt::Debug for NoiseSuppressor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NoiseSuppressor")
            .field("spec", &self.spec)
            .finish_non_exhaustive()
    }
}

impl PartialEq for NoiseSuppressor {
    fn eq(&self, other: &Self) -> bool {
        self.spec == other.spec
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NoiseSuppressorSpec {
    amount: f32,
}

impl NoiseSuppressorSpec {
    pub fn new(amount_percent: u32) -> Self {
        Self {
            amount: amount_percent.min(100) as f32 / 100.0,
        }
    }
}

impl NoiseSuppressor {
    pub fn new(spec: NoiseSuppressorSpec) -> Self {
        assert_eq!(DenoiseState::FRAME_SIZE, BLOCK_FRAMES);
        Self {
            spec,
            states: std::array::from_fn(|_| DenoiseState::new()),
            input: [[0.0; BLOCK_FRAMES]; CHANNELS],
            output: [[0.0; BLOCK_FRAMES]; CHANNELS],
        }
    }

    pub fn adopt(&mut self, spec: NoiseSuppressorSpec) {
        self.spec = spec;
    }

    /// Cleans one mixer block in place. The RNNoise binding uses f32 to carry
    /// signed 16-bit PCM values, while Pubsplash uses normalized f32 samples.
    pub fn process(&mut self, block: &mut [f32]) {
        if block.len() != BLOCK_FRAMES * CHANNELS {
            log::error!("Noise suppressor received a block with the wrong length");
            return;
        }
        for (frame_index, frame) in block.chunks_exact(CHANNELS).enumerate() {
            for channel in 0..CHANNELS {
                self.input[channel][frame_index] = frame[channel].clamp(-1.0, 1.0) * PCM_SCALE;
            }
        }
        for channel in 0..CHANNELS {
            self.states[channel].process_frame(&mut self.output[channel], &self.input[channel]);
        }
        for (frame_index, frame) in block.chunks_exact_mut(CHANNELS).enumerate() {
            for channel in 0..CHANNELS {
                let cleaned = self.output[channel][frame_index] / PCM_SCALE;
                let mixed = frame[channel] + (cleaned - frame[channel]) * self.spec.amount;
                frame[channel] = if mixed.is_finite() { mixed } else { 0.0 };
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_amount_leaves_audio_unchanged() {
        let mut suppressor = NoiseSuppressor::new(NoiseSuppressorSpec::new(0));
        let mut block = (0..BLOCK_FRAMES * CHANNELS)
            .map(|sample| sample as f32 / 1_000.0)
            .collect::<Vec<_>>();
        let original = block.clone();
        suppressor.process(&mut block);
        assert_eq!(block, original);
    }

    #[test]
    fn corrupted_amount_is_clamped() {
        assert_eq!(NoiseSuppressorSpec::new(101).amount, 1.0);
    }
}
