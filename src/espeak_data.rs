//! Unpacking of the espeak-ng data embedded in the executable.
//!
//! espeak-ng only knows how to read its `espeak-ng-data/` from disk, so on the
//! first run we dump the embedded archive into the user's cache and hand that
//! path over through `PIPER_ESPEAKNG_DATA_DIRECTORY`.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};

const ARCHIVE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/espeak-data.gz"));
const HASH: &str = env!("MCPIPER_ESPEAK_HASH");
const MAGIC: &[u8; 5] = b"MCPD1";
const READY: &str = ".ready";

/// Languages included in this binary, exactly as requested at build time.
pub const LANGS: &str = env!("MCPIPER_ESPEAK_LANGS_BUILT");

/// Makes `espeak-ng-data` available on disk and returns its *parent* directory,
/// which is what `PIPER_ESPEAKNG_DATA_DIRECTORY` expects.
pub fn ensure(override_dir: Option<&Path>) -> Result<PathBuf> {
    if let Some(dir) = override_dir {
        return resolve_override(dir);
    }

    let base = cache_root()?.join(format!("espeak-{HASH}"));
    if base.join(READY).is_file() && base.join("espeak-ng-data").join("phontab").is_file() {
        return Ok(base);
    }

    // Unpack into a staging directory and rename, so two concurrent processes
    // cannot trample each other halfway through writing.
    let staging = base.with_extension(format!("tmp{}", std::process::id()));
    let _ = fs::remove_dir_all(&staging);
    unpack(&staging.join("espeak-ng-data"))
        .with_context(|| format!("unpacking espeak-ng-data into {}", staging.display()))?;
    fs::write(staging.join(READY), HASH)?;

    if let Some(parent) = base.parent() {
        fs::create_dir_all(parent)?;
    }
    match fs::rename(&staging, &base) {
        Ok(()) => {}
        // Another process got there first: its copy is identical (the hash is in the name).
        Err(_) if base.join(READY).is_file() => {
            let _ = fs::remove_dir_all(&staging);
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&staging);
            return Err(e).context("installing espeak-ng-data into the cache");
        }
    }
    Ok(base)
}

/// Accepts either `.../espeak-ng-data` or the directory containing it.
fn resolve_override(dir: &Path) -> Result<PathBuf> {
    if dir.join("espeak-ng-data").join("phontab").is_file() {
        return Ok(dir.to_path_buf());
    }
    if dir.join("phontab").is_file() {
        return dir
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| anyhow!("`{}` has no parent directory", dir.display()));
    }
    bail!(
        "`{}` does not look like a valid espeak-ng-data (no `phontab` found)",
        dir.display()
    )
}

fn cache_root() -> Result<PathBuf> {
    let dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("mcpiper");
    fs::create_dir_all(&dir)
        .with_context(|| format!("creating the cache at {}", dir.display()))?;
    Ok(dir)
}

fn unpack(dest: &Path) -> Result<()> {
    let mut raw = Vec::new();
    flate2::read::GzDecoder::new(ARCHIVE).read_to_end(&mut raw)?;

    let mut cur = Reader { buf: &raw, pos: 0 };
    if cur.take(MAGIC.len())? != MAGIC {
        bail!("the embedded espeak-ng archive is corrupt");
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
        fs::write(&path, data).with_context(|| format!("writing {}", path.display()))?;
    }
    Ok(())
}

/// We generate the archive ourselves, but still reject paths that escape the destination.
fn safe_join(dest: &Path, name: &str) -> Result<PathBuf> {
    let mut path = dest.to_path_buf();
    for part in name.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("invalid path in the embedded archive: `{name}`");
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
            .ok_or_else(|| anyhow!("the embedded espeak-ng archive is truncated"))?;
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
