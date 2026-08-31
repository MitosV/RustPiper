//! mcpiper — Piper text-to-speech en un único ejecutable, sin instalar nada.

mod audio;
mod espeak_data;
mod synth;

use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{bail, Context, Result};
use clap::Parser;

use audio::{Format, Rate};
use synth::Options;

/// Convierte texto a voz con un modelo Piper y lo exporta a Ogg Vorbis o WAV.
#[derive(Parser, Debug)]
#[command(
    name = "mcpiper",
    version,
    about = "Piper text-to-speech en un solo ejecutable",
    after_help = "Ejemplos:\n  \
        mcpiper --model ./model/ana --text \"Hola\" -o ./out.ogg\n  \
        echo \"Hola mundo\" | mcpiper -m ./model/ana.onnx -o - > out.ogg\n  \
        mcpiper -m ./model/ana --list-speakers"
)]
struct Args {
    /// Modelo Piper: `voz`, `voz.onnx` o un directorio que contenga uno.
    #[arg(short, long, value_name = "RUTA", required_unless_present = "self_test")]
    model: Option<PathBuf>,

    /// JSON de configuración (por defecto `<modelo>.onnx.json`).
    #[arg(short, long, value_name = "RUTA")]
    config: Option<PathBuf>,

    /// Texto a sintetizar. Si se omite, se lee de la entrada estándar.
    #[arg(short, long, value_name = "TEXTO", conflicts_with = "text_file")]
    text: Option<String>,

    /// Archivo de texto a sintetizar.
    #[arg(short = 'f', long, value_name = "RUTA")]
    text_file: Option<PathBuf>,

    /// Archivo de salida. `-` escribe a la salida estándar.
    #[arg(
        short,
        long,
        value_name = "RUTA",
        required_unless_present_any = ["list_speakers", "self_test"]
    )]
    output: Option<PathBuf>,

    /// Formato de salida. Por defecto se deduce de la extensión (.ogg → Vorbis).
    #[arg(long, value_enum, value_name = "FORMATO")]
    format: Option<Format>,

    /// Calidad VBR de Vorbis, de -0.2 (mínima) a 1.0 (máxima). Por defecto 0.3.
    #[arg(
        long,
        value_name = "Q",
        allow_negative_numbers = true,
        conflicts_with = "bitrate",
        value_parser = quality_in_range
    )]
    quality: Option<f32>,

    /// Bitrate medio de Vorbis en bits por segundo, en vez de apuntar a una calidad.
    #[arg(long, value_name = "BPS", value_parser = clap::value_parser!(u32).range(8_000..=500_000))]
    bitrate: Option<u32>,

    /// Hablante a usar, por nombre o por número (modelos multi-voz).
    #[arg(short, long, value_name = "NOMBRE|ID")]
    speaker: Option<String>,

    /// Lista los hablantes del modelo y sale.
    #[arg(long)]
    list_speakers: bool,

    /// Velocidad: >1 más lento, <1 más rápido.
    #[arg(long, value_name = "F")]
    length_scale: Option<f32>,

    /// Variabilidad de la entonación.
    #[arg(long, value_name = "F")]
    noise_scale: Option<f32>,

    /// Variabilidad de la duración de cada fonema.
    #[arg(long, value_name = "F")]
    noise_w: Option<f32>,

    /// Pausa entre frases, en segundos.
    #[arg(long, value_name = "SEG", default_value_t = 0.2)]
    sentence_silence: f32,

    /// El texto de entrada ya son fonemas IPA.
    #[arg(long)]
    phonemes: bool,

    /// Verifica que el ejecutable funcione en esta máquina, sin necesitar un modelo.
    #[arg(long)]
    self_test: bool,

    /// Usar un `espeak-ng-data` externo en vez del embebido.
    #[arg(long, value_name = "DIR")]
    espeak_data: Option<PathBuf>,

    /// No imprimir nada en la salida de error salvo fallos.
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("mcpiper: {e:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args = Args::parse();

    if args.self_test {
        return self_test(args.espeak_data.as_deref());
    }

    let model_arg = args.model.clone().expect("clap lo exige salvo con --self-test");
    let model = synth::resolve_model(&model_arg)?;
    let config = match args.config {
        Some(p) => {
            if !p.is_file() {
                bail!("no encontré la configuración `{}`", p.display());
            }
            p
        }
        None => synth::config_path_for(&model).with_context(|| {
            format!(
                "no encontré la configuración del modelo; probé `{}.json` y `{}`",
                model.display(),
                model.with_extension("json").display()
            )
        })?,
    };

    // espeak-ng lee sus datos del disco, así que los volcamos antes de tocar nada más.
    let data_dir = espeak_data::ensure(args.espeak_data.as_deref())?;
    std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &data_dir);

    let mut voice = synth::Voice::load(&model, &config)?;

    if args.list_speakers {
        print_speakers(&voice);
        return Ok(());
    }

    let text = read_text(args.text.as_deref(), args.text_file.as_deref())?;
    if text.trim().is_empty() {
        bail!("no hay texto para sintetizar (usá --text, --text-file o la entrada estándar)");
    }

    let output = args.output.expect("clap lo exige salvo con --list-speakers");
    let to_stdout = output.as_os_str() == "-";
    let format = pick_format(args.format, &output, to_stdout)?;

    let opts = Options {
        speaker_id: match args.speaker.as_deref() {
            Some(spec) => Some(voice.resolve_speaker(spec)?),
            None => None,
        },
        length_scale: args.length_scale,
        noise_scale: args.noise_scale,
        noise_w: args.noise_w,
        sentence_silence: args.sentence_silence,
        input_is_phonemes: args.phonemes,
    };

    let rate = match (args.quality, args.bitrate) {
        (Some(q), _) => Rate::Quality(q),
        (None, Some(bps)) => Rate::Bitrate(bps),
        (None, None) => Rate::default(),
    };

    let started = std::time::Instant::now();
    let samples = voice.synthesize(&text, &opts)?;
    let sample_rate = voice.sample_rate();
    let bytes = audio::encode(format, &samples, sample_rate, rate)?;

    if to_stdout {
        let mut stdout = std::io::stdout().lock();
        if stdout.is_terminal() {
            bail!("me negué a volcar audio binario a la terminal; redirigí la salida a un archivo");
        }
        stdout.write_all(&bytes)?;
        stdout.flush()?;
    } else {
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creando `{}`", parent.display()))?;
        }
        std::fs::write(&output, &bytes)
            .with_context(|| format!("escribiendo `{}`", output.display()))?;
    }

    if !args.quiet {
        let seconds = samples.len() as f32 / sample_rate as f32;
        let elapsed = started.elapsed().as_secs_f32();
        eprintln!(
            "mcpiper: {seconds:.2}s de audio en {elapsed:.2}s ({:.1}x tiempo real) -> {} [{} KiB]",
            seconds / elapsed.max(f32::EPSILON),
            if to_stdout { "stdout".into() } else { output.display().to_string() },
            bytes.len() / 1024,
        );
    }
    Ok(())
}

/// Vorbis acepta calidades fuera de [-0.2, 1.0] con resultados raros; las cortamos acá.
fn quality_in_range(raw: &str) -> Result<f32, String> {
    let q: f32 = raw.parse().map_err(|_| format!("`{raw}` no es un número"))?;
    if (-0.2..=1.0).contains(&q) {
        Ok(q)
    } else {
        Err(format!("la calidad va de -0.2 a 1.0, no {q}"))
    }
}

fn pick_format(explicit: Option<Format>, output: &std::path::Path, to_stdout: bool) -> Result<Format> {
    if let Some(f) = explicit {
        return Ok(f);
    }
    if to_stdout {
        return Ok(Format::Vorbis);
    }
    let ext = output.extension().and_then(|e| e.to_str());
    Format::from_extension(ext).ok_or_else(|| match ext {
        Some(e) => {
            anyhow::anyhow!("no sé qué hacer con la extensión `.{e}`; usá --format vorbis|wav")
        }
        None => anyhow::anyhow!("`{}` no tiene extensión; usá --format vorbis|wav", output.display()),
    })
}

fn read_text(inline: Option<&str>, file: Option<&std::path::Path>) -> Result<String> {
    if let Some(t) = inline {
        return Ok(t.to_string());
    }
    if let Some(p) = file {
        return std::fs::read_to_string(p)
            .with_context(|| format!("leyendo `{}`", p.display()));
    }
    let mut stdin = std::io::stdin().lock();
    if stdin.is_terminal() {
        bail!("no hay texto para sintetizar (usá --text, --text-file o pipeá algo por stdin)");
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf).context("leyendo la entrada estándar")?;
    Ok(buf)
}

/// Ejercita todo lo que no depende del modelo: extraer los datos de espeak-ng,
/// fonemizar, y producir un Ogg Vorbis válido. Sirve para validar el binario en
/// una plataforma nueva sin bajar 60 MB de pesos.
fn self_test(espeak_dir: Option<&std::path::Path>) -> Result<()> {
    println!("mcpiper {}", env!("CARGO_PKG_VERSION"));

    let data_dir = espeak_data::ensure(espeak_dir)?;
    std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &data_dir);
    println!("espeak-ng-data : {} (idiomas: {})", data_dir.display(), espeak_data::LANGS);

    for (lang, sample) in [("es", "Hola mundo."), ("en-us", "Hello world.")] {
        let phonemes = espeak_rs::text_to_phonemes(sample, lang, None)
            .map_err(|e| anyhow::anyhow!("fonemizando en `{lang}`: {e}"))?
            .join(" ");
        if phonemes.trim().is_empty() {
            bail!("espeak-ng no devolvió fonemas para `{lang}`");
        }
        println!("fonemas {lang:<6}: {sample} -> {phonemes}");
    }

    // Un segundo de tono a 22050 Hz, el mismo camino que sigue el audio real.
    let tone: Vec<f32> = (0..22_050)
        .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 22_050.0).sin() * 0.5)
        .collect();
    let ogg = audio::encode(Format::Vorbis, &tone, 22_050, Rate::default())?;
    if &ogg[0..4] != b"OggS" || !ogg.windows(7).any(|w| w == b"\x01vorbis") {
        bail!("el codificador Ogg Vorbis produjo un archivo inválido");
    }
    println!("ogg vorbis     : {} bytes para 1,00 s de tono", ogg.len());

    let wav = audio::encode(Format::Wav, &tone, 22_050, Rate::default())?;
    println!("wav            : {} bytes", wav.len());

    println!("\ntodo en orden.");
    Ok(())
}

fn print_speakers(voice: &synth::Voice) {
    println!("voz espeak-ng : {}", voice.espeak_voice());
    println!("sample rate   : {} Hz", voice.sample_rate());
    println!("hablantes     : {}", voice.num_speakers());
    println!("espeak-ng     : idiomas embebidos = {}", espeak_data::LANGS);

    let speakers = voice.speakers();
    if speakers.is_empty() {
        println!("\n(modelo de una sola voz; --speaker no aplica)");
        return;
    }
    let mut rows: Vec<(&String, &i64)> = speakers.iter().collect();
    rows.sort_by_key(|(_, id)| **id);
    println!();
    for (name, id) in rows {
        println!("{id:>4}  {name}");
    }
}
