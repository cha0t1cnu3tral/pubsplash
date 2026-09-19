//! Streaming encoders: MP3 via LAME and AAC-LC in ADTS via FDK AAC.

use crate::audio::mixer::{CHANNELS, SAMPLE_RATE};
use crate::config::StreamFormat;
use fdk_aac::enc::{
    AudioObjectType, BitRate, ChannelMode, Encoder as FdkEncoder, EncoderParams, Transport,
};
use mp3lame_encoder::{Bitrate, Builder, Encoder, FlushNoGap, InterleavedPcm};

/// One of the elementary-stream encoders Pubsplash can hand to Icecast.
pub enum StreamEncoder {
    Mp3(Mp3Encoder),
    Aac(AacEncoder),
}

impl StreamEncoder {
    pub fn new(format: StreamFormat, bitrate_kbps: u32) -> Result<Self, String> {
        match format {
            StreamFormat::Mp3 => Mp3Encoder::new(bitrate_kbps).map(Self::Mp3),
            StreamFormat::Aac => AacEncoder::new(bitrate_kbps).map(Self::Aac),
        }
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Result<&[u8], String> {
        match self {
            Self::Mp3(encoder) => encoder.encode(pcm),
            Self::Aac(encoder) => encoder.encode(pcm),
        }
    }

    pub fn finish(self) -> Result<Vec<u8>, String> {
        match self {
            Self::Mp3(encoder) => encoder.finish(),
            Self::Aac(encoder) => encoder.finish(),
        }
    }
}

pub struct Mp3Encoder {
    encoder: Encoder,
    out: Vec<u8>,
}

fn bitrate_from_kbps(kbps: u32) -> Bitrate {
    match kbps {
        0..=48 => Bitrate::Kbps48,
        49..=64 => Bitrate::Kbps64,
        65..=96 => Bitrate::Kbps96,
        97..=128 => Bitrate::Kbps128,
        129..=160 => Bitrate::Kbps160,
        161..=192 => Bitrate::Kbps192,
        193..=256 => Bitrate::Kbps256,
        _ => Bitrate::Kbps320,
    }
}

impl Mp3Encoder {
    pub fn new(bitrate_kbps: u32) -> Result<Self, String> {
        let mut builder = Builder::new().ok_or("failed to allocate LAME encoder")?;
        builder
            .set_num_channels(CHANNELS as u8)
            .map_err(|e| e.to_string())?;
        builder
            .set_sample_rate(SAMPLE_RATE)
            .map_err(|e| e.to_string())?;
        builder
            .set_brate(bitrate_from_kbps(bitrate_kbps))
            .map_err(|e| e.to_string())?;
        builder
            .set_quality(mp3lame_encoder::Quality::Good)
            .map_err(|e| e.to_string())?;
        let encoder = builder.build().map_err(|e| e.to_string())?;
        Ok(Self {
            encoder,
            out: Vec::new(),
        })
    }

    /// Encodes one interleaved i16 block; returns the MP3 bytes produced
    /// (possibly empty while LAME buffers).
    pub fn encode(&mut self, pcm: &[i16]) -> Result<&[u8], String> {
        self.out.clear();
        self.out.reserve(mp3lame_encoder::max_required_buffer_size(
            pcm.len() / CHANNELS,
        ));
        let written = self
            .encoder
            .encode(InterleavedPcm(pcm), self.out.spare_capacity_mut())
            .map_err(|e| e.to_string())?;
        // SAFETY: `encode` initialized `written` bytes of the spare capacity.
        unsafe { self.out.set_len(written) };
        Ok(&self.out)
    }

    /// Flushes LAME's internal buffer at end of stream.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        self.out.clear();
        self.out.reserve(16 * 1024);
        let written = self
            .encoder
            .flush::<FlushNoGap>(self.out.spare_capacity_mut())
            .map_err(|e| e.to_string())?;
        // SAFETY: `flush` initialized `written` bytes of the spare capacity.
        unsafe { self.out.set_len(written) };
        Ok(self.out)
    }
}

/// AAC-LC encoder emitting ADTS frames, the format Icecast expects for an
/// `audio/aac` mount. AAC works in 1024-frame units while the mixer works in
/// 10 ms units, so this owns the small PCM accumulator between calls.
pub struct AacEncoder {
    encoder: FdkEncoder,
    frame_samples: usize,
    pending: Vec<i16>,
    out: Vec<u8>,
}

impl AacEncoder {
    pub fn new(bitrate_kbps: u32) -> Result<Self, String> {
        let encoder = FdkEncoder::new(EncoderParams {
            bit_rate: BitRate::Cbr(bitrate_kbps.saturating_mul(1000)),
            sample_rate: SAMPLE_RATE,
            transport: Transport::Adts,
            channels: ChannelMode::Stereo,
            audio_object_type: AudioObjectType::Mpeg4LowComplexity,
        })
        .map_err(|e| e.to_string())?;
        let info = encoder.info().map_err(|e| e.to_string())?;
        let frame_samples = (info.frameLength as usize) * CHANNELS;
        if frame_samples == 0 {
            return Err("AAC encoder reported an empty frame size".into());
        }
        Ok(Self {
            encoder,
            frame_samples,
            pending: Vec::with_capacity(frame_samples * 2),
            // FDK's maximum AAC-LC ADTS frame is comfortably below this.
            out: Vec::with_capacity(16 * 1024),
        })
    }

    pub fn encode(&mut self, pcm: &[i16]) -> Result<&[u8], String> {
        self.pending.extend_from_slice(pcm);
        self.out.clear();
        while self.pending.len() >= self.frame_samples {
            self.encode_frame()?;
        }
        Ok(&self.out)
    }

    fn encode_frame(&mut self) -> Result<(), String> {
        let mut encoded = [0u8; 16 * 1024];
        let info = self
            .encoder
            .encode(&self.pending[..self.frame_samples], &mut encoded)
            .map_err(|e| e.to_string())?;
        if info.input_consumed != self.frame_samples {
            return Err(format!(
                "AAC encoder consumed {} samples from a {}-sample frame",
                info.input_consumed, self.frame_samples
            ));
        }
        self.pending.drain(..self.frame_samples);
        self.out.extend_from_slice(&encoded[..info.output_size]);
        Ok(())
    }

    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        // `out` held the preceding call's bytes, which the mixer already sent.
        // Only return the padded tail produced while closing.
        self.out.clear();
        // Pad the incomplete 10 ms mixer block so the final real samples are
        // encoded. An AAC-LC frame is 21.3 ms at 48 kHz.
        if !self.pending.is_empty() {
            self.pending.resize(self.frame_samples, 0);
            self.encode_frame()?;
        }
        Ok(self.out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::mixer::BLOCK_SAMPLES;

    #[test]
    fn encodes_sine_to_valid_mp3_frames() {
        let mut enc = Mp3Encoder::new(128).unwrap();
        // 1 second of 440 Hz.
        let mut produced = Vec::new();
        let mut t = 0f32;
        for _ in 0..100 {
            let block: Vec<i16> = (0..BLOCK_SAMPLES)
                .map(|i| {
                    if i % 2 == 0 {
                        t += 1.0 / SAMPLE_RATE as f32;
                    }
                    ((t * 440.0 * std::f32::consts::TAU).sin() * 16000.0) as i16
                })
                .collect();
            produced.extend_from_slice(enc.encode(&block).unwrap());
        }
        produced.extend(enc.finish().unwrap());
        assert!(
            produced.len() > 4000,
            "should produce a meaningful bitstream"
        );
        // MP3 frame sync: 11 set bits.
        let sync = produced
            .windows(2)
            .position(|w| w[0] == 0xFF && (w[1] & 0xE0) == 0xE0);
        assert!(sync.is_some(), "no MP3 frame sync found");
    }

    #[test]
    fn encodes_sine_to_adts_aac_frames() {
        let mut enc = AacEncoder::new(128).unwrap();
        let pcm = vec![0i16; 48_000 * CHANNELS];
        let mut produced = enc.encode(&pcm).unwrap().to_vec();
        produced.extend(enc.finish().unwrap());
        assert!(
            produced.len() > 7,
            "AAC stream should contain an ADTS frame"
        );
        assert_eq!(&produced[..2], &[0xff, 0xf1], "missing ADTS sync word");
    }

    #[test]
    fn finishing_aac_does_not_repeat_the_last_sent_frame() {
        let mut enc = AacEncoder::new(128).unwrap();
        let pcm = vec![0i16; enc.frame_samples];
        assert!(!enc.encode(&pcm).unwrap().is_empty());
        assert!(enc.finish().unwrap().is_empty());
    }
}
