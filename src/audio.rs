//! Output encoding: Ogg Vorbis (the default) and WAV.

use std::io::Cursor;
use std::num::{NonZeroU32, NonZeroU8};

use anyhow::{Context, Result};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

/// How many samples we hand to libvorbis at a time. It does not change the
/// result, it just avoids keeping a duplicate of the whole block in the
/// internal buffers.
const BLOCK: usize = 4096;

/// Default VBR quality, on the Vorbis scale (-0.2 to 1.0). It matches `oggenc`'s
/// `-q 3` and gives ~52 kbps for mono speech at 22050 Hz: transparent for
/// synthesized speech without spending bytes for nothing.
pub const DEFAULT_QUALITY: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Ogg Vorbis.
    Vorbis,
    /// Uncompressed 16-bit PCM WAV.
    Wav,
}

impl Format {
    /// Infers the format from the output file's extension.
    pub fn from_extension(ext: Option<&str>) -> Option<Self> {
        match ext?.to_ascii_lowercase().as_str() {
            "ogg" | "oga" => Some(Self::Vorbis),
            "wav" | "wave" => Some(Self::Wav),
            _ => None,
        }
    }
}

/// How the size of the Ogg Vorbis file is chosen.
#[derive(Clone, Copy, Debug)]
pub enum Rate {
    /// VBR by perceptual quality: the encoder spends whatever it takes.
    Quality(f32),
    /// VBR aiming at an average bitrate, in bits per second.
    Bitrate(u32),
}

impl Default for Rate {
    fn default() -> Self {
        Self::Quality(DEFAULT_QUALITY)
    }
}

pub fn encode(format: Format, samples: &[f32], sample_rate: u32, rate: Rate) -> Result<Vec<u8>> {
    match format {
        Format::Vorbis => encode_ogg_vorbis(samples, sample_rate, rate),
        Format::Wav => encode_wav(samples, sample_rate),
    }
}

fn encode_wav(samples: &[f32], sample_rate: u32) -> Result<Vec<u8>> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut buf = Cursor::new(Vec::new());
    {
        let mut writer = hound::WavWriter::new(&mut buf, spec).context("creating the WAV")?;
        for s in samples {
            writer.write_sample(to_i16(*s))?;
        }
        writer.finalize().context("closing the WAV")?;
    }
    Ok(buf.into_inner())
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

/// Vorbis encodes at the model's native frequency (22050 Hz for most Piper
/// voices), so no resampling is involved.
fn encode_ogg_vorbis(samples: &[f32], sample_rate: u32, rate: Rate) -> Result<Vec<u8>> {
    let frequency =
        NonZeroU32::new(sample_rate).context("the model declares a sample rate of 0 Hz")?;
    let mono = NonZeroU8::new(1).expect("1 is not zero");

    let strategy = match rate {
        Rate::Quality(q) => VorbisBitrateManagementStrategy::QualityVbr {
            target_quality: q,
        },
        Rate::Bitrate(bps) => VorbisBitrateManagementStrategy::Vbr {
            target_bitrate: NonZeroU32::new(bps).context("the bitrate cannot be 0")?,
        },
    };

    // The serial is derived from the content instead of being drawn at random,
    // so that the same audio always produces the same file byte for byte.
    let serial = serial_for(samples.len(), sample_rate);
    let mut builder =
        VorbisEncoderBuilder::new_with_serial(frequency, mono, Cursor::new(Vec::new()), serial);
    builder.bitrate_management_strategy(strategy);
    builder
        .comment_tag("ENCODER", concat!("mcpiper ", env!("CARGO_PKG_VERSION")))
        .context("writing the Ogg tags")?;
    let mut encoder = builder.build().map_err(|e| match rate {
        // libvorbis only ships managed-bitrate modes for certain ranges, and
        // which ones depends on the sample rate. When the request falls outside,
        // the error it returns is `OV_EIMPL`, which tells nobody anything.
        Rate::Bitrate(bps) => bitrate_out_of_range(bps, sample_rate),
        Rate::Quality(_) => anyhow::Error::new(e).context("initializing the Vorbis encoder"),
    })?;

    for block in samples.chunks(BLOCK) {
        encoder
            .encode_audio_block([block])
            .context("encoding an audio block")?;
    }

    Ok(encoder
        .finish()
        .context("closing the Ogg Vorbis stream")?
        .into_inner())
}

fn bitrate_out_of_range(requested: u32, sample_rate: u32) -> anyhow::Error {
    match supported_bitrate_range(sample_rate) {
        Some((min, max)) => anyhow::anyhow!(
            "Vorbis has no {} kbps mode for {} Hz mono; the usable range is {}-{} kbps.\n  \
             Use --quality instead if you want to pin the quality and let the bitrate settle.",
            requested / 1000,
            sample_rate,
            min / 1000,
            max / 1000
        ),
        None => anyhow::anyhow!(
            "Vorbis cannot encode {} Hz mono at a fixed bitrate; use --quality.",
            sample_rate
        ),
    }
}

/// Finds out by trial and error which bitrates libvorbis accepts for this sample
/// rate. It is only called once an encode has already failed, so the cost is moot.
fn supported_bitrate_range(sample_rate: u32) -> Option<(u32, u32)> {
    let frequency = NonZeroU32::new(sample_rate)?;
    let mono = NonZeroU8::new(1)?;

    let accepts = |bps: u32| -> bool {
        let Some(target_bitrate) = NonZeroU32::new(bps) else {
            return false;
        };
        let mut builder =
            VorbisEncoderBuilder::new_with_serial(frequency, mono, std::io::sink(), 1);
        builder.bitrate_management_strategy(VorbisBitrateManagementStrategy::Vbr {
            target_bitrate,
        });
        builder.build().is_ok()
    };

    let supported: Vec<u32> = (8..=320)
        .step_by(8)
        .map(|k| k * 1000)
        .filter(|b| accepts(*b))
        .collect();
    Some((*supported.first()?, *supported.last()?))
}

/// The serial identifies the stream inside the Ogg container.
fn serial_for(len: usize, sample_rate: u32) -> i32 {
    let mut hash: u32 = 2_166_136_261;
    for b in (len as u64)
        .to_le_bytes()
        .iter()
        .chain(sample_rate.to_le_bytes().iter())
    {
        hash ^= *b as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    // The field is a signed i32; stay in the positive range.
    (hash >> 1) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(samples: usize, hz: f32, sample_rate: u32) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                (2.0 * std::f32::consts::PI * hz * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect()
    }

    #[test]
    fn ogg_vorbis_carries_the_format_headers() {
        let bytes =
            encode_ogg_vorbis(&tone(22_050, 440.0, 22_050), 22_050, Rate::default()).unwrap();
        assert_eq!(&bytes[0..4], b"OggS");
        // The three Vorbis header packets: identification, comments and setup.
        assert!(bytes.windows(7).any(|w| w == b"\x01vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"\x03vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"\x05vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"mcpiper"));
    }

    #[test]
    fn the_bitrate_drives_the_size() {
        let audio = tone(22_050 * 3, 440.0, 22_050);
        let small = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(24_000)).unwrap();
        let large = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(80_000)).unwrap();
        assert!(
            large.len() * 2 > small.len() * 3,
            "80 kbps ({}) should weigh a good deal more than 24 kbps ({})",
            large.len(),
            small.len()
        );
    }

    #[test]
    fn an_impossible_bitrate_explains_the_real_range() {
        let audio = tone(22_050, 440.0, 22_050);
        let e = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(200_000)).unwrap_err();
        let msg = format!("{e}");
        // libvorbis decides the exact range; what matters is that we report it.
        assert!(msg.contains("24-"), "the message carries no range: {msg}");
        assert!(msg.contains("22050 Hz"), "the message does not name the sample rate: {msg}");
        assert!(msg.contains("--quality"), "the message does not suggest the alternative: {msg}");
    }

    #[test]
    fn the_output_is_deterministic() {
        let audio = tone(22_050, 440.0, 22_050);
        let a = encode_ogg_vorbis(&audio, 22_050, Rate::default()).unwrap();
        let b = encode_ogg_vorbis(&audio, 22_050, Rate::default()).unwrap();
        assert_eq!(a, b, "two encodes of the same audio should match");
    }

    #[test]
    fn the_wav_keeps_the_model_sample_rate() {
        let bytes = encode_wav(&tone(1_000, 440.0, 16_000), 16_000).unwrap();
        let read = hound::WavReader::new(Cursor::new(&bytes)).unwrap();
        assert_eq!(read.spec().sample_rate, 16_000);
        assert_eq!(read.spec().channels, 1);
        assert_eq!(read.len(), 1_000);
    }

    #[test]
    fn format_from_extension() {
        assert_eq!(Format::from_extension(Some("ogg")), Some(Format::Vorbis));
        assert_eq!(Format::from_extension(Some("WAV")), Some(Format::Wav));
        assert_eq!(Format::from_extension(Some("mp3")), None);
    }
}
