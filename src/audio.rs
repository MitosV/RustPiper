//! Codificación de la salida: Ogg Vorbis (por defecto) y WAV.

use std::io::Cursor;
use std::num::{NonZeroU32, NonZeroU8};

use anyhow::{Context, Result};
use vorbis_rs::{VorbisBitrateManagementStrategy, VorbisEncoderBuilder};

/// Cuántas muestras le pasamos a libvorbis de una. No cambia el resultado,
/// sólo evita tener el bloque entero duplicado en los búferes internos.
const BLOCK: usize = 4096;

/// Calidad VBR por defecto, en la escala de Vorbis (-0.2 a 1.0). Equivale al
/// `-q 3` de `oggenc` y da ~52 kbps sobre voz mono a 22050 Hz: transparente
/// para habla sintetizada, sin gastar bytes de más.
pub const DEFAULT_QUALITY: f32 = 0.3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Format {
    /// Ogg Vorbis.
    Vorbis,
    /// WAV PCM 16 bits, sin comprimir.
    Wav,
}

impl Format {
    /// Deduce el formato por la extensión del archivo de salida.
    pub fn from_extension(ext: Option<&str>) -> Option<Self> {
        match ext?.to_ascii_lowercase().as_str() {
            "ogg" | "oga" => Some(Self::Vorbis),
            "wav" | "wave" => Some(Self::Wav),
            _ => None,
        }
    }
}

/// Cómo se elige el tamaño del Ogg Vorbis.
#[derive(Clone, Copy, Debug)]
pub enum Rate {
    /// VBR por calidad perceptual: el codificador gasta lo que haga falta.
    Quality(f32),
    /// VBR apuntando a un bitrate medio, en bits por segundo.
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
        let mut writer = hound::WavWriter::new(&mut buf, spec).context("creando el WAV")?;
        for s in samples {
            writer.write_sample(to_i16(*s))?;
        }
        writer.finalize().context("cerrando el WAV")?;
    }
    Ok(buf.into_inner())
}

fn to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

/// Vorbis codifica a la frecuencia nativa del modelo (22050 Hz en la mayoría de
/// las voces Piper), así que no hay remuestreo de por medio.
fn encode_ogg_vorbis(samples: &[f32], sample_rate: u32, rate: Rate) -> Result<Vec<u8>> {
    let frequency = NonZeroU32::new(sample_rate)
        .context("el modelo declara un sample rate de 0 Hz")?;
    let mono = NonZeroU8::new(1).expect("1 no es cero");

    let strategy = match rate {
        Rate::Quality(q) => VorbisBitrateManagementStrategy::QualityVbr {
            target_quality: q,
        },
        Rate::Bitrate(bps) => VorbisBitrateManagementStrategy::Vbr {
            target_bitrate: NonZeroU32::new(bps).context("el bitrate no puede ser 0")?,
        },
    };

    // El serial se deriva del contenido en vez de sortearse, para que el mismo
    // audio produzca siempre el mismo archivo byte a byte.
    let serial = serial_for(samples.len(), sample_rate);
    let mut builder =
        VorbisEncoderBuilder::new_with_serial(frequency, mono, Cursor::new(Vec::new()), serial);
    builder.bitrate_management_strategy(strategy);
    builder
        .comment_tag("ENCODER", concat!("mcpiper ", env!("CARGO_PKG_VERSION")))
        .context("escribiendo las etiquetas del Ogg")?;
    let mut encoder = builder.build().map_err(|e| match rate {
        // libvorbis sólo trae modos de bitrate manejado para ciertos rangos, y
        // cuáles depende del sample rate. Cuando el pedido cae afuera, el error
        // que devuelve es `OV_EIMPL`, que no le dice nada a nadie.
        Rate::Bitrate(bps) => bitrate_fuera_de_rango(bps, sample_rate),
        Rate::Quality(_) => anyhow::Error::new(e).context("inicializando el codificador Vorbis"),
    })?;

    for block in samples.chunks(BLOCK) {
        encoder
            .encode_audio_block([block])
            .context("codificando un bloque de audio")?;
    }

    Ok(encoder
        .finish()
        .context("cerrando el flujo Ogg Vorbis")?
        .into_inner())
}

fn bitrate_fuera_de_rango(pedido: u32, sample_rate: u32) -> anyhow::Error {
    match rango_de_bitrate_soportado(sample_rate) {
        Some((min, max)) => anyhow::anyhow!(
            "Vorbis no tiene un modo de {} kbps para {} Hz mono; el rango utilizable \
             es {}-{} kbps.\n  Usá --quality en su lugar si querés fijar la calidad \
             y dejar que el bitrate se acomode.",
            pedido / 1000,
            sample_rate,
            min / 1000,
            max / 1000
        ),
        None => anyhow::anyhow!(
            "Vorbis no puede codificar a {} Hz mono con bitrate fijo; usá --quality.",
            sample_rate
        ),
    }
}

/// Averigua a prueba y error qué bitrates acepta libvorbis para este sample rate.
/// Sólo se llama cuando ya falló una codificación, así que el costo no importa.
fn rango_de_bitrate_soportado(sample_rate: u32) -> Option<(u32, u32)> {
    let frequency = NonZeroU32::new(sample_rate)?;
    let mono = NonZeroU8::new(1)?;

    let acepta = |bps: u32| -> bool {
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

    let soportados: Vec<u32> = (8..=320).step_by(8).map(|k| k * 1000).filter(|b| acepta(*b)).collect();
    Some((*soportados.first()?, *soportados.last()?))
}

/// El serial identifica el flujo dentro del Ogg.
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
    // El campo es i32 con signo; nos quedamos siempre en el rango positivo.
    (hash >> 1) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tono(muestras: usize, hz: f32, sample_rate: u32) -> Vec<f32> {
        (0..muestras)
            .map(|i| {
                (2.0 * std::f32::consts::PI * hz * i as f32 / sample_rate as f32).sin() * 0.5
            })
            .collect()
    }

    #[test]
    fn ogg_vorbis_tiene_las_cabeceras_del_formato() {
        let bytes = encode_ogg_vorbis(&tono(22_050, 440.0, 22_050), 22_050, Rate::default())
            .unwrap();
        assert_eq!(&bytes[0..4], b"OggS");
        // Los tres paquetes de cabecera de Vorbis: identificación, comentarios y setup.
        assert!(bytes.windows(7).any(|w| w == b"\x01vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"\x03vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"\x05vorbis"));
        assert!(bytes.windows(7).any(|w| w == b"mcpiper"));
    }

    #[test]
    fn el_bitrate_manda_sobre_el_tamano() {
        let audio = tono(22_050 * 3, 440.0, 22_050);
        let chico = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(24_000)).unwrap();
        let grande = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(80_000)).unwrap();
        assert!(
            grande.len() * 2 > chico.len() * 3,
            "80 kbps ({}) debería pesar bastante más que 24 kbps ({})",
            grande.len(),
            chico.len()
        );
    }

    #[test]
    fn un_bitrate_imposible_explica_el_rango_real() {
        let audio = tono(22_050, 440.0, 22_050);
        let e = encode_ogg_vorbis(&audio, 22_050, Rate::Bitrate(200_000)).unwrap_err();
        let msg = format!("{e}");
        // El rango exacto lo decide libvorbis; nos importa que lo informe.
        assert!(msg.contains("24-"), "el mensaje no trae el rango: {msg}");
        assert!(msg.contains("22050 Hz"), "el mensaje no nombra el sample rate: {msg}");
        assert!(msg.contains("--quality"), "el mensaje no sugiere la alternativa: {msg}");
    }

    #[test]
    fn la_salida_es_determinista() {
        let audio = tono(22_050, 440.0, 22_050);
        let a = encode_ogg_vorbis(&audio, 22_050, Rate::default()).unwrap();
        let b = encode_ogg_vorbis(&audio, 22_050, Rate::default()).unwrap();
        assert_eq!(a, b, "dos codificaciones del mismo audio deberían coincidir");
    }

    #[test]
    fn el_wav_respeta_el_sample_rate_del_modelo() {
        let bytes = encode_wav(&tono(1_000, 440.0, 16_000), 16_000).unwrap();
        let leido = hound::WavReader::new(Cursor::new(&bytes)).unwrap();
        assert_eq!(leido.spec().sample_rate, 16_000);
        assert_eq!(leido.spec().channels, 1);
        assert_eq!(leido.len(), 1_000);
    }

    #[test]
    fn formato_por_extension() {
        assert_eq!(Format::from_extension(Some("ogg")), Some(Format::Vorbis));
        assert_eq!(Format::from_extension(Some("WAV")), Some(Format::Wav));
        assert_eq!(Format::from_extension(Some("mp3")), None);
    }
}

