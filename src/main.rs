//! mcpiper — Piper text-to-speech in a single executable, with nothing to install.

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

/// Turns text into speech with a Piper model and writes Ogg Vorbis or WAV.
#[derive(Parser, Debug)]
#[command(
    name = "mcpiper",
    version,
    about = "Piper text-to-speech in a single executable",
    after_help = "Examples:\n  \
        mcpiper --model ./model/ana --text \"Hello\" -o ./out.ogg\n  \
        echo \"Hello world\" | mcpiper -m ./model/ana.onnx -o - > out.ogg\n  \
        mcpiper -m ./model/ana --list-speakers"
)]
struct Args {
    /// Piper model: `voice`, `voice.onnx`, or a directory containing one.
    #[arg(short, long, value_name = "PATH", required_unless_present = "self_test")]
    model: Option<PathBuf>,

    /// Configuration JSON (defaults to `<model>.onnx.json`).
    #[arg(short, long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// Text to synthesize. If omitted, it is read from standard input.
    #[arg(short, long, value_name = "TEXT", conflicts_with = "text_file")]
    text: Option<String>,

    /// Text file to synthesize.
    #[arg(short = 'f', long, value_name = "PATH")]
    text_file: Option<PathBuf>,

    /// Output file. `-` writes to standard output.
    #[arg(
        short,
        long,
        value_name = "PATH",
        required_unless_present_any = ["list_speakers", "self_test"]
    )]
    output: Option<PathBuf>,

    /// Output format. Inferred from the extension by default (.ogg → Vorbis).
    #[arg(long, value_enum, value_name = "FORMAT")]
    format: Option<Format>,

    /// Vorbis VBR quality, from -0.2 (lowest) to 1.0 (highest). Defaults to 0.3.
    #[arg(
        long,
        value_name = "Q",
        allow_negative_numbers = true,
        conflicts_with = "bitrate",
        value_parser = quality_in_range
    )]
    quality: Option<f32>,

    /// Average Vorbis bitrate in bits per second, instead of targeting a quality.
    #[arg(long, value_name = "BPS", value_parser = clap::value_parser!(u32).range(8_000..=500_000))]
    bitrate: Option<u32>,

    /// Speaker to use, by name or by number (multi-voice models).
    #[arg(short, long, value_name = "NAME|ID")]
    speaker: Option<String>,

    /// List the model's speakers and exit.
    #[arg(long)]
    list_speakers: bool,

    /// Speed: >1 slower, <1 faster.
    #[arg(long, value_name = "F")]
    length_scale: Option<f32>,

    /// Intonation variability.
    #[arg(long, value_name = "F")]
    noise_scale: Option<f32>,

    /// Per-phoneme duration variability.
    #[arg(long, value_name = "F")]
    noise_w: Option<f32>,

    /// Pause between sentences, in seconds.
    #[arg(long, value_name = "SEC", default_value_t = 0.2)]
    sentence_silence: f32,

    /// The input text is already IPA phonemes.
    #[arg(long)]
    phonemes: bool,

    /// Check that the executable works on this machine, without needing a model.
    #[arg(long)]
    self_test: bool,

    /// Use an external `espeak-ng-data` instead of the embedded one.
    #[arg(long, value_name = "DIR")]
    espeak_data: Option<PathBuf>,

    /// Print nothing on standard error except failures.
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

    let model_arg = args.model.clone().expect("clap requires it except with --self-test");
    let model = synth::resolve_model(&model_arg)?;
    let config = match args.config {
        Some(p) => {
            if !p.is_file() {
                bail!("could not find the configuration `{}`", p.display());
            }
            p
        }
        None => synth::config_path_for(&model).with_context(|| {
            format!(
                "could not find the model's configuration; tried `{}.json` and `{}`",
                model.display(),
                model.with_extension("json").display()
            )
        })?,
    };

    // espeak-ng reads its data from disk, so unpack it before touching anything else.
    let data_dir = espeak_data::ensure(args.espeak_data.as_deref())?;
    std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &data_dir);

    let mut voice = synth::Voice::load(&model, &config)?;

    if args.list_speakers {
        print_speakers(&voice);
        return Ok(());
    }

    let text = read_text(args.text.as_deref(), args.text_file.as_deref())?;
    if text.trim().is_empty() {
        bail!("nothing to synthesize (use --text, --text-file or standard input)");
    }

    let output = args.output.expect("clap requires it except with --list-speakers");
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
            bail!("refusing to dump binary audio to the terminal; redirect the output to a file");
        }
        stdout.write_all(&bytes)?;
        stdout.flush()?;
    } else {
        if let Some(parent) = output.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating `{}`", parent.display()))?;
        }
        std::fs::write(&output, &bytes)
            .with_context(|| format!("writing `{}`", output.display()))?;
    }

    if !args.quiet {
        let seconds = samples.len() as f32 / sample_rate as f32;
        let elapsed = started.elapsed().as_secs_f32();
        eprintln!(
            "mcpiper: {seconds:.2}s of audio in {elapsed:.2}s ({:.1}x realtime) -> {} [{} KiB]",
            seconds / elapsed.max(f32::EPSILON),
            if to_stdout { "stdout".into() } else { output.display().to_string() },
            bytes.len() / 1024,
        );
    }
    Ok(())
}

/// Vorbis accepts qualities outside [-0.2, 1.0] with odd results; clamp them here.
fn quality_in_range(raw: &str) -> Result<f32, String> {
    let q: f32 = raw.parse().map_err(|_| format!("`{raw}` is not a number"))?;
    if (-0.2..=1.0).contains(&q) {
        Ok(q)
    } else {
        Err(format!("quality ranges from -0.2 to 1.0, not {q}"))
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
            anyhow::anyhow!("don't know what to do with the `.{e}` extension; use --format vorbis|wav")
        }
        None => {
            anyhow::anyhow!("`{}` has no extension; use --format vorbis|wav", output.display())
        }
    })
}

fn read_text(inline: Option<&str>, file: Option<&std::path::Path>) -> Result<String> {
    if let Some(t) = inline {
        return Ok(t.to_string());
    }
    if let Some(p) = file {
        return std::fs::read_to_string(p).with_context(|| format!("reading `{}`", p.display()));
    }
    let mut stdin = std::io::stdin().lock();
    if stdin.is_terminal() {
        bail!("nothing to synthesize (use --text, --text-file or pipe something into stdin)");
    }
    let mut buf = String::new();
    stdin.read_to_string(&mut buf).context("reading standard input")?;
    Ok(buf)
}

/// Exercises everything that does not depend on the model: unpacking the espeak-ng
/// data, phonemizing, and producing a valid Ogg Vorbis file. Useful to validate the
/// binary on a new platform without downloading 60 MB of weights.
fn self_test(espeak_dir: Option<&std::path::Path>) -> Result<()> {
    println!("mcpiper {}", env!("CARGO_PKG_VERSION"));

    let data_dir = espeak_data::ensure(espeak_dir)?;
    std::env::set_var("PIPER_ESPEAKNG_DATA_DIRECTORY", &data_dir);
    println!("espeak-ng-data : {} (languages: {})", data_dir.display(), espeak_data::LANGS);

    for (lang, sample) in [("es", "Hola mundo."), ("en-us", "Hello world.")] {
        let phonemes = espeak_rs::text_to_phonemes(sample, lang, None)
            .map_err(|e| anyhow::anyhow!("phonemizing in `{lang}`: {e}"))?
            .join(" ");
        if phonemes.trim().is_empty() {
            bail!("espeak-ng returned no phonemes for `{lang}`");
        }
        println!("phonemes {lang:<5}: {sample} -> {phonemes}");
    }

    // One second of tone at 22050 Hz, down the same path real audio takes.
    let tone: Vec<f32> = (0..22_050)
        .map(|i| (2.0 * std::f32::consts::PI * 220.0 * i as f32 / 22_050.0).sin() * 0.5)
        .collect();
    let ogg = audio::encode(Format::Vorbis, &tone, 22_050, Rate::default())?;
    if &ogg[0..4] != b"OggS" || !ogg.windows(7).any(|w| w == b"\x01vorbis") {
        bail!("the Ogg Vorbis encoder produced an invalid file");
    }
    println!("ogg vorbis     : {} bytes for 1.00 s of tone", ogg.len());

    let wav = audio::encode(Format::Wav, &tone, 22_050, Rate::default())?;
    println!("wav            : {} bytes", wav.len());

    println!("\nall good.");
    Ok(())
}

fn print_speakers(voice: &synth::Voice) {
    println!("espeak-ng voice : {}", voice.espeak_voice());
    println!("sample rate     : {} Hz", voice.sample_rate());
    println!("speakers        : {}", voice.num_speakers());
    println!("espeak-ng       : embedded languages = {}", espeak_data::LANGS);

    let speakers = voice.speakers();
    if speakers.is_empty() {
        println!("\n(single-voice model; --speaker does not apply)");
        return;
    }
    let mut rows: Vec<(&String, &i64)> = speakers.iter().collect();
    rows.sort_by_key(|(_, id)| **id);
    println!();
    for (name, id) in rows {
        println!("{id:>4}  {name}");
    }
}
