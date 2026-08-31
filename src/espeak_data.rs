//! Extracción de los datos de espeak-ng embebidos en el ejecutable.
//!
//! espeak-ng solo sabe leer su `espeak-ng-data/` desde el disco, así que en el
//! primer arranque volcamos el archivo embebido a la caché del usuario y le
//! pasamos esa ruta por `PIPER_ESPEAKNG_DATA_DIRECTORY`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

const ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/espeak-data.gz"));
const HASH: &str = env!("MCPIPER_ESPEAK_HASH");
const MAGIC: &[u8; 5] = b"MCPD1";
const READY: &str = ".ready";

/// Idiomas incluidos en este binario, tal como se pidieron en tiempo de compilación.
pub const LANGS: &str = env!("MCPIPER_ESPEAK_LANGS_BUILT");

/// Deja `espeak-ng-data` disponible en disco y devuelve el directorio *padre*,
/// que es lo que espera `PIPER_ESPEAKNG_DATA_DIRECTORY`.
pub fn ensure(override_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = override_dir {
        return resolve_override(dir);
    }

    let base = cache_root()?.join(format!("espeak-{HASH}"));
    if base.join(READY).is_file() && base.join("espeak-ng-data").join("phontab").is_file() {
        return Ok(base);
    }

    // Extraemos a un directorio temporal y renombramos, para que dos procesos
    // simultáneos no se pisen a mitad de la escritura.
    let staging = base.with_extension(format!("tmp{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    unpack(&staging.join("espeak-ng-data"))
        .with_context(|| format!("extrayendo espeak-ng-data en {}", staging.display()))?;
    fs::write(staging.join(READY), HASH)?;

    if let Some(parent) = base.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(&staging, &base) {
        Ok(()) => {}
        // Otro proceso llegó primero: su copia es idéntica (el hash está en el nombre).
        Err(_) if base.join(READY).is_file() => {
            let _ = fs::remove_dir_all(&staging);
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e).context("instalando espeak-ng-data en la caché");
        }
    }
    Ok(base)
}

/// Acepta tanto `.../espeak-ng-data` como el directorio que lo contiene.
fn resolve_override(dir: &Path) -> Result<PathBuf> {
    if dir.join("espeak-ng-data").join("phontab").is_file() {
        return Ok(dir.to_path_buf());
    }
    if dir.join("phontab").is_file() {
        return dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("`{}` no tiene directorio padre", dir.display()));
    }
    bail!(
        "`{}` no parece un espeak-ng-data válido (no encontré `phontab`)",
        dir.display()
    )
}

fn cache_root() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("mcpiper");
    fs::create_dir_all(&dir)
        .with_context(|| format!("creando la caché en {}", dir.display()))?;
    Ok(dir)
}

fn unpack(dest: &Path) -> Result<()> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(ARCHIVE).read_to_end(&mut raw)?;

    let mut cur = Reader { buf: &raw, pos: 0 };
    if cur.take(MAGIC.len())? != MAGIC {
        bail!("el archivo embebido de espeak-ng está corrupto");
    }
    let count = cur.u32()?;

    fs::create_dir_all(dest)?;
    for _ in 0..count {
        let name_len = cur.u16()? as usize;
        let name = std::str::from_utf8(cur.take(name_len)?)?.to_string();
        let data_len = cur.u32()? as usize;
        let data = cur.take(data_len)?;

        let path = safe_join(dest, &name)?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(&path, data).with_context(|| format!("escribiendo {}", path.display()))?;
    }
    Ok(())
}

/// El archivo lo generamos nosotros, pero igual rechazamos rutas que se escapen del destino.
fn safe_join(dest: &Path, name: &str) -> Result<PathBuf> {
    let mut path = dest.to_path_buf();
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("ruta inválida en el archivo embebido: `{name}`");
        }
        path.push(part);
    }
    Ok(path)
}

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self
            .pos
            .checked_add(n)
            .filter(|e| *e <= self.buf.len())
            .ok_or_else(|| anyhow!("el archivo embebido de espeak-ng está truncado"))?;
        let slice = &self.buf[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
}
