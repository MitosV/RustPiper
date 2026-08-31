//! Carga del modelo Piper y síntesis frase por frase.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use piper_rs::{ModelConfig, Piper};

/// Parámetros de generación. `None` = usar lo que traiga el modelo en su JSON.
#[derive(Debug, Default, Clone, Copy)]
pub struct Options {
    pub speaker_id: Option<i64>,
    pub length_scale: Option<f32>,
    pub noise_scale: Option<f32>,
    pub noise_w: Option<f32>,
    /// Silencio insertado entre frases, en segundos.
    pub sentence_silence: f32,
    /// El texto de entrada ya son fonemas IPA: saltearse espeak-ng.
    pub input_is_phonemes: bool,
}

pub struct Voice {
    piper: Piper,
    /// Segunda copia del JSON: `Piper` no expone su configuración interna.
    config: ModelConfig,
}

impl Voice {
    pub fn load(model: &Path, config_path: &Path) -> Result<Self> {
        let json = std::fs::read_to_string(config_path)
            .with_context(|| format!("leyendo la configuración `{}`", config_path.display()))?;
        // `Piper` se queda con su propia `ModelConfig` sin exponerla, así que
        // mantenemos una copia nuestra para los metadatos (voz, hablantes, rate).
        let config: ModelConfig = serde_json::from_str(&json)
            .with_context(|| format!("interpretando `{}`", config_path.display()))?;
        let piper = Piper::new(model, config_path).map_err(|e| anyhow!("{e}"))?;
        let voice = Self { piper, config };
        voice.check_espeak_voice()?;
        Ok(voice)
    }

    /// Falla temprano si el binario no trae el diccionario del idioma del modelo.
    ///
    /// Cuando falta, espeak-ng no devuelve error: escribe una queja por stderr y
    /// entrega cero fonemas, con lo que el fallo aparecería recién al sintetizar
    /// y disfrazado de "texto vacío". Lo detectamos con una palabra de prueba.
    fn check_espeak_voice(&self) -> Result<()> {
        let voice = &self.config.espeak.voice;
        let probe = espeak_rs::text_to_phonemes("abcde", voice, None)
            .map_err(|e| anyhow!("el modelo pide la voz espeak-ng `{voice}`: {e}"))?;
        if probe.iter().any(|s| !s.trim().is_empty()) {
            return Ok(());
        }
        bail!(
            "este binario no trae los datos de espeak-ng para la voz `{voice}` que pide el modelo \
             (incluye: {}).\n  \
             Opciones: recompilar con MCPIPER_ESPEAK_LANGS=\"{},{}\", o pasar \
             --espeak-data con un espeak-ng-data completo del sistema.",
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

    /// Traduce `--speaker` (nombre o número) al id que espera el modelo.
    pub fn resolve_speaker(&self, spec: &str) -> Result<i64> {
        if let Some(id) = self.config.speaker_id_map.get(spec) {
            return Ok(*id);
        }
        if let Ok(id) = spec.parse::<i64>() {
            if id >= 0 && (id as u32) < self.config.num_speakers.max(1) {
                return Ok(id);
            }
            bail!(
                "el hablante {id} está fuera de rango: el modelo tiene {}",
                self.config.num_speakers
            );
        }
        let mut known: Vec<&str> = self.config.speaker_id_map.keys().map(String::as_str).collect();
        known.sort_unstable();
        if known.is_empty() {
            bail!("este modelo tiene una sola voz, `--speaker` no aplica");
        }
        bail!("no conozco al hablante `{spec}`. Disponibles: {}", known.join(", "))
    }

    /// Divide el texto en frases con espeak-ng y sintetiza cada una por separado,
    /// para poder meter una pausa natural entre ellas.
    pub fn synthesize(&mut self, text: &str, opts: &Options) -> Result<Vec<f32>> {
        let sentences: Vec<String> = if opts.input_is_phonemes {
            vec![text.to_string()]
        } else {
            espeak_rs::text_to_phonemes(text, self.config.espeak.voice.as_str(), None)
                .map_err(|e| anyhow!("fonemizando el texto: {e}"))?
        };
        let sentences: Vec<String> = sentences
            .into_iter()
            .filter(|s| !s.trim().is_empty())
            .collect();
        if sentences.is_empty() {
            bail!("el texto no produjo ningún fonema; ¿está vacío o es solo puntuación?");
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
                .map_err(|e| anyhow!("sintetizando la frase {}: {e}", i + 1))?;
            audio.extend_from_slice(&chunk);
        }
        Ok(audio)
    }
}

/// Acepta `voz`, `voz.onnx` o un directorio con un único `.onnx` adentro.
pub fn resolve_model(spec: &Path) -> Result<PathBuf> {
    if spec.is_file() {
        return Ok(spec.to_path_buf());
    }
    if spec.is_dir() {
        let mut found: Vec<PathBuf> = std::fs::read_dir(spec)
            .with_context(|| format!("listando `{}`", spec.display()))?
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "onnx"))
            .collect();
        found.sort();
        return match found.len() {
            1 => Ok(found.remove(0)),
            0 => bail!("`{}` no contiene ningún archivo .onnx", spec.display()),
            n => bail!(
                "`{}` contiene {n} modelos .onnx; indicá cuál con --model ruta/al/modelo.onnx",
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
        "no encontré el modelo: probé `{}` y `{}`",
        spec.display(),
        with_ext.display()
    )
}

/// Piper publica la configuración como `<modelo>.onnx.json`; algunos paquetes
/// la distribuyen como `<modelo>.json`.
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
