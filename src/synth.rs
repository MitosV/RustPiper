//! Piper model loading and sentence-by-sentence synthesis.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use piper_rs::{ModelConfig, Piper};

/// Generation parameters. `None` = use whatever the model's JSON carries.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub speaker_id: Option<i64>,
    pub length_scale: Option<f32>,
    pub noise_scale: Option<f32>,
    pub noise_w: Option<f32>,
    /// Silence inserted between sentences, in seconds.
    pub sentence_silence: f32,
    /// The input text is already IPA phonemes: skip espeak-ng.
    pub input_is_phonemes: bool,
}

pub struct Voice {
    piper: Piper,
    /// A second copy of the JSON: `Piper` does not expose its internal config.
    config: ModelConfig,
}

impl Voice {
    pub fn load(model: &Path, config_path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(config_path)
            .with_context(|| format!("reading the configuration `{}`", config_path.display()))?;
        // `Piper` keeps its own `ModelConfig` without exposing it, so we hold a
        // copy of our own for the metadata (voice, speakers, sample rate).
        let config: ModelConfig = serde_json::from_str(&json)
            .with_context(|| format!("parsing `{}`", config_path.display()))?;
        let piper = Piper::new(model, config_path).map_err(|e| anyhow!("{e}"))?;
        let voice = Self { piper, config };
        voice.check_espeak_voice()?;
        Ok(voice)
    }

    /// Fails early if the binary does not carry the dictionary for the model's language.
    ///
    /// When it is missing, espeak-ng does not return an error: it writes a complaint
    /// to stderr and hands back zero phonemes, so the failure would only surface at
    /// synthesis time, disguised as "empty text". We catch it with a probe word.
    fn check_espeak_voice(&self) -> Result<()> {
        let voice = &self.config.espeak.voice;
        let probe = espeak_rs::text_to_phonemes("abcde", voice, None)
            .map_err(|e| anyhow!("the model asks for the espeak-ng voice `{voice}`: {e}"))?;
        if probe.iter().any(|s| !s.trim().is_empty()) {
            return Ok(());
        }
        bail!(
            "this binary does not carry the espeak-ng data for the voice `{voice}` the model \
             asks for (it includes: {}).\n  \
             Options: rebuild with MCPIPER_ESPEAK_LANGS=\"{},{}\", or pass --espeak-data \
             pointing at a full espeak-ng-data from the system.",
            crate::espeak_data::LANGS,
            crate::espeak_data::LANGS,
            voice.split(['-', '_']).next().unwrap_or(voice)
        )
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.audio.sample_rate
    }

    pub fn espeak_voice(&self) -> &str {
        &self.config.espeak.voice
    }

    pub fn speakers(&self) -> &HashMap<String, i64> {
        &self.config.speaker_id_map
    }

    pub fn num_speakers(&self) -> u32 {
        self.config.num_speakers
    }

    /// Translates `--speaker` (a name or a number) into the id the model expects.
    pub fn resolve_speaker(&self, spec: &str) -> Result<i64> {
        if let Some(id) = self.config.speaker_id_map.get(spec) {
            return Ok(*id);
        }
        if let Ok(id) = spec.parse::<i64>() {
            if id >= 0 && (id as u32) < self.config.num_speakers.max(1) {
                return Ok(id);
            }
            bail!(
                "speaker {id} is out of range: the model has {}",
                self.config.num_speakers
            );
        }
        let mut known: Vec<&str> = self.config.speaker_id_map.keys().map(String::as_str).collect();
        known.sort_unstable();
        if known.is_empty() {
            bail!("this model has a single voice, `--speaker` does not apply");
        }
        bail!("unknown speaker `{spec}`. Available: {}", known.join(", "))
    }

    /// Splits the text into sentences with espeak-ng and synthesizes each one
    /// separately, so a natural pause can be placed between them.
    pub fn synthesize(&mut self, text: &str, opts: &Options) -> Result<Vec<f32>> {
        let sentences: Vec<String> = if opts.input_is_phonemes {
            vec![text.to_string()]
        } else {
            espeak_rs::text_to_phonemes(text, self.config.espeak.voice.as_str(), None)
                .map_err(|e| anyhow!("phonemizing the text: {e}"))?
        };
        let sentences: Vec<String> = sentences
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        if sentences.is_empty() {
            bail!("the text produced no phonemes; is it empty, or only punctuation?");
        }

        let gap = (self.config.audio.sample_rate as f32 * opts.sentence_silence.max(0.0)) as usize;
        let mut audio = Vec::new();
        for (i, phonemes) in sentences.iter().enumerate() {
            if i > 0 {
                audio.resize(audio.len() + gap, 0.0);
            }
            let (chunk, _) = self
                .piper
                .create(
                    phonemes,
                    true,
                    opts.speaker_id,
                    opts.length_scale,
                    opts.noise_scale,
                    opts.noise_w,
                )
                .map_err(|e| anyhow!("synthesizing sentence {}: {e}", i + 1))?;
            audio.extend_from_slice(&chunk);
        }
        Ok(audio)
    }
}

/// Accepts `voice`, `voice.onnx`, or a directory holding a single `.onnx`.
pub fn resolve_model(spec: &Path) -> Result<PathBuf> {
    if spec.is_file() {
        return Ok(spec.to_path_buf());
    }
    if spec.is_dir() {
        let mut found: Vec<PathBuf> = std::fs::read_dir(spec)
            .with_context(|| format!("listing `{}`", spec.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "onnx"))
            .collect();
        found.sort();
        return match found.len() {
            1 => Ok(found.remove(0)),
            0 => bail!("`{}` contains no .onnx file", spec.display()),
            n => bail!(
                "`{}` contains {n} .onnx models; say which one with --model path/to/model.onnx",
                spec.display()
            ),
        };
    }

    // `--model ./model/ana` -> `./model/ana.onnx`
    let with_ext = append_extension(spec, "onnx");
    if with_ext.is_file() {
        return Ok(with_ext);
    }
    bail!(
        "could not find the model: tried `{}` and `{}`",
        spec.display(),
        with_ext.display()
    )
}

/// Piper publishes the configuration as `<model>.onnx.json`; some packages
/// distribute it as `<model>.json`.
pub fn config_path_for(model: &Path) -> Option<PathBuf> {
    let candidates = [
        append_extension(model, "json"),
        model.with_extension("json"),
    ];
    candidates.into_iter().find(|p| p.is_file())
}

fn append_extension(path: &Path, ext: &str) -> PathBuf {
    let mut name = path.as_os_str().to_os_string();
    name.push(".");
    name.push(ext);
    PathBuf::from(name)
}
