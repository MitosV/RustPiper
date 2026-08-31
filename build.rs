//! Compila los datos de espeak-ng dentro del binario.
//!
//! `espeak-rs-sys` construye espeak-ng con CMake y deja `espeak-ng-data/` en su
//! `OUT_DIR`. Acá lo localizamos, filtramos los diccionarios por idioma, y
//! empaquetamos todo en un archivo gzip que `include_bytes!` mete en el ejecutable.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

const MAGIC: &[u8; 5] = b"MCPD1";

fn main() {
    println!("cargo:rerun-if-env-changed=MCPIPER_ESPEAK_LANGS");
    println!("cargo:rerun-if-env-changed=MCPIPER_ESPEAK_DATA_DIR");

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let data_dir = locate_espeak_data(&out_dir).unwrap_or_else(|| {
        panic!(
            "no encontré `espeak-ng-data`. Se esperaba que el build de espeak-rs-sys \
             lo generara bajo el directorio `build/` de cargo. Podés apuntarlo a mano \
             con MCPIPER_ESPEAK_DATA_DIR=/ruta/a/espeak-ng-data"
        )
    });

    let langs = std::env::var("MCPIPER_ESPEAK_LANGS").unwrap_or_else(|_| "es,en".to_string());
    let filter = LangFilter::parse(&langs);

    let mut files = Vec::new();
    collect(&data_dir, &data_dir, &filter, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));
    assert!(!files.is_empty(), "espeak-ng-data está vacío: {}", data_dir.display());

    let mut raw = Vec::new();
    raw.extend_from_slice(MAGIC);
    raw.extend_from_slice(&(files.len() as u32).to_le_bytes());
    for (rel, abs) in &files {
        let bytes = fs::read(abs).unwrap_or_else(|e| panic!("leyendo {}: {e}", abs.display()));
        raw.extend_from_slice(&(rel.len() as u16).to_le_bytes());
        raw.extend_from_slice(rel.as_bytes());
        raw.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        raw.extend_from_slice(&bytes);
    }

    let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
    enc.write_all(&raw).expect("comprimiendo espeak-ng-data");
    let packed = enc.finish().expect("comprimiendo espeak-ng-data");

    let dest = out_dir.join("espeak-data.gz");
    fs::write(&dest, &packed).expect("escribiendo espeak-data.gz");

    println!("cargo:rustc-env=MCPIPER_ESPEAK_HASH={:016x}", fnv1a(&packed));
    println!("cargo:rustc-env=MCPIPER_ESPEAK_LANGS_BUILT={langs}");
    println!(
        "cargo:warning=espeak-ng-data: {} archivos, {} KiB en crudo -> {} KiB embebidos (idiomas: {langs})",
        files.len(),
        raw.len() / 1024,
        packed.len() / 1024
    );
}

/// Qué diccionarios de idioma se embeben. `all` los incluye todos.
enum LangFilter {
    All,
    Only(Vec<String>),
}

impl LangFilter {
    fn parse(spec: &str) -> Self {
        if spec.trim().eq_ignore_ascii_case("all") {
            return Self::All;
        }
        Self::Only(
            spec.split(',')
                .map(|s| s.trim().to_ascii_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    /// Los `*_dict` son el 80% del peso; el resto (phondata, lang/, voices/) va siempre.
    fn keeps_dict(&self, lang: &str) -> bool {
        match self {
            Self::All => true,
            // `es` habilita `es_dict`; un modelo `es-419` usa igual ese diccionario.
            Self::Only(list) => list.iter().any(|l| l == lang),
        }
    }
}

fn collect(root: &Path, dir: &Path, filter: &LangFilter, out: &mut Vec<(String, PathBuf)>) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(root, &path, filter, out);
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .expect("ruta dentro de espeak-ng-data")
            .to_string_lossy()
            .replace('\\', "/");
        if let Some(lang) = rel.strip_suffix("_dict") {
            if lang.contains('/') || filter.keeps_dict(lang) {
                out.push((rel, path));
            }
            continue;
        }
        out.push((rel, path));
    }
}

/// Busca `espeak-ng-data` en el `OUT_DIR` de espeak-rs-sys, que es hermano del nuestro.
fn locate_espeak_data(our_out_dir: &Path) -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("MCPIPER_ESPEAK_DATA_DIR") {
        let p = PathBuf::from(dir);
        if p.is_dir() {
            return Some(p);
        }
    }

    // OUT_DIR = <target>/<profile>/build/mcpiper-<hash>/out
    let build_dir = our_out_dir.parent()?.parent()?;
    let mut candidates: Vec<PathBuf> = fs::read_dir(build_dir)
        .ok()?
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("espeak-rs-sys-"))
        })
        .collect();
    // Varias carpetas `espeak-rs-sys-*` conviven (script vs salida); preferimos la más nueva.
    candidates.sort();
    candidates.reverse();

    for base in candidates {
        // Orden de preferencia: el `install` de CMake primero, que es el canónico.
        for rel in ["out/share/espeak-ng-data", "out/build/espeak-ng-data"] {
            let p = base.join(rel);
            if p.join("phontab").is_file() {
                println!("cargo:rerun-if-changed={}", p.display());
                return Some(p);
            }
        }
    }
    None
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x1000_0000_01b3);
    }
    hash
}
