//! Zip archives, unpacked safely: what the lab downloads (src/download.rs) is unpacked only inside the folder it is
//! given. An entry is refused when its path could leave that folder (`..`, an absolute path, a drive, a backslash),
//! when it is a link, encrypted, or compressed by a method other than storing or deflating, and when the archive
//! holds more entries or bytes than its limits; every file's CRC-32 is checked. ZIP64 archives (an entry or the
//! archive past 4 GiB) are refused.

use anyhow::{Context, Result, anyhow, bail};
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// The most an archive may hold.
#[derive(Clone, Copy)]
pub struct Limits {
    /// Entries, files and folders.
    pub entries: usize,
    /// Bytes once unpacked, every file together.
    pub bytes: u64,
}

/// An entry of the central directory.
struct Entry {
    name: String,
    method: u16,
    crc: u32,
    compressed: u64,
    size: u64,
    /// Where its local header is.
    offset: u64,
}

const END: u32 = 0x0605_4b50;
const CENTRAL: u32 = 0x0201_4b50;
const LOCAL: u32 = 0x0403_4b50;

fn u16_at(bytes: &[u8], at: usize) -> Result<u16> {
    let b = bytes.get(at..at + 2).ok_or_else(|| anyhow!("truncated"))?;
    Ok(u16::from_le_bytes([b[0], b[1]]))
}

fn u32_at(bytes: &[u8], at: usize) -> Result<u32> {
    let b = bytes.get(at..at + 4).ok_or_else(|| anyhow!("truncated"))?;
    Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// The archive's central directory: where it starts and how many entries it has, from the end record.
fn central_directory(file: &mut File) -> Result<(u64, u64, usize)> {
    let length = file.metadata()?.len();
    // The end record is the last 22 bytes, followed by a comment of at most 65535.
    let tail = length.min(22 + 65535);
    file.seek(SeekFrom::Start(length - tail))?;
    let mut bytes = vec![0; tail as usize];
    file.read_exact(&mut bytes)?;
    let at = (0..bytes.len().saturating_sub(21))
        .rev()
        .find(|&i| u32_at(&bytes, i).is_ok_and(|s| s == END))
        .ok_or_else(|| anyhow!("not a zip archive (no end record)"))?;
    let entries = u16_at(&bytes, at + 10)?;
    let (size, offset) = (u32_at(&bytes, at + 12)?, u32_at(&bytes, at + 16)?);
    if entries == u16::MAX || size == u32::MAX || offset == u32::MAX {
        bail!("ZIP64 archives are not read");
    }
    Ok((u64::from(offset), u64::from(size), usize::from(entries)))
}

/// The central directory's entries, refusing links and encrypted entries.
fn entries(file: &mut File, limits: Limits) -> Result<Vec<Entry>> {
    let (offset, size, count) = central_directory(file)?;
    if count > limits.entries {
        bail!("{count} entries, more than {}", limits.entries);
    }
    file.seek(SeekFrom::Start(offset))?;
    let mut bytes = vec![0; size as usize];
    file.read_exact(&mut bytes)?;
    let mut out = Vec::with_capacity(count);
    let mut at = 0;
    for _ in 0..count {
        if u32_at(&bytes, at)? != CENTRAL {
            bail!("a broken central directory");
        }
        let name_length = usize::from(u16_at(&bytes, at + 28)?);
        let skip = usize::from(u16_at(&bytes, at + 30)?) + usize::from(u16_at(&bytes, at + 32)?);
        let name = bytes
            .get(at + 46..at + 46 + name_length)
            .ok_or_else(|| anyhow!("truncated"))?;
        let name = String::from_utf8(name.to_vec()).context("an entry's name is not UTF-8")?;
        // Made on Unix (the high byte 3): its mode is in the attributes' high half, and a link is refused.
        let unix = u16_at(&bytes, at + 4)? >> 8 == 3;
        let mode = u32_at(&bytes, at + 38)? >> 16;
        if unix && mode & 0o170_000 == 0o120_000 {
            bail!("`{name}` is a link");
        }
        if u16_at(&bytes, at + 8)? & 1 != 0 {
            bail!("`{name}` is encrypted");
        }
        let entry = Entry {
            method: u16_at(&bytes, at + 10)?,
            crc: u32_at(&bytes, at + 16)?,
            compressed: u64::from(u32_at(&bytes, at + 20)?),
            size: u64::from(u32_at(&bytes, at + 24)?),
            offset: u64::from(u32_at(&bytes, at + 42)?),
            name,
        };
        if [entry.compressed, entry.size, entry.offset].contains(&u64::from(u32::MAX)) {
            bail!("`{}`: ZIP64 entries are not read", entry.name);
        }
        out.push(entry);
        at += 46 + name_length + skip;
    }
    let total: u64 = out.iter().map(|e| e.size).sum();
    if total > limits.bytes {
        bail!("{total} bytes unpacked, more than {}", limits.bytes);
    }
    Ok(out)
}

/// An entry's name as a path inside the folder it is unpacked into, or an error when it could leave it.
pub fn safe_path(name: &str) -> Result<PathBuf> {
    let name = name.strip_suffix('/').unwrap_or(name);
    let bad = name.is_empty()
        || name.starts_with('/')
        || name.contains(['\\', ':', '\0'])
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        bail!("`{name}` is not a path inside the archive's folder");
    }
    Ok(name.split('/').collect())
}

/// Unpacks one file entry to `target`, checking its size and CRC-32.
fn unpack_file(file: &mut File, entry: &Entry, target: &Path) -> Result<()> {
    file.seek(SeekFrom::Start(entry.offset))?;
    let mut header = [0; 30];
    file.read_exact(&mut header)?;
    if u32_at(&header, 0)? != LOCAL {
        bail!("`{}`: a broken local header", entry.name);
    }
    let skip = u64::from(u16_at(&header, 26)?) + u64::from(u16_at(&header, 28)?);
    file.seek(SeekFrom::Current(skip as i64))?;
    let data = Read::by_ref(file).take(entry.compressed);
    let mut reader: Box<dyn Read + '_> = match entry.method {
        0 => Box::new(data),
        8 => Box::new(flate2::read::DeflateDecoder::new(data)),
        m => bail!("`{}`: compression method {m} is not read", entry.name),
    };
    let mut out = File::create(target).with_context(|| format!("create {}", target.display()))?;
    let mut crc = flate2::Crc::new();
    let mut buffer = vec![0; 1 << 16];
    let mut written = 0u64;
    loop {
        let n = reader.read(&mut buffer)?;
        if n == 0 {
            break;
        }
        // Never past the size the directory gives: a lying entry stops here.
        written += n as u64;
        if written > entry.size {
            bail!("`{}` is larger than its entry says", entry.name);
        }
        crc.update(&buffer[..n]);
        std::io::Write::write_all(&mut out, &buffer[..n])?;
    }
    if written != entry.size || crc.sum() != entry.crc {
        bail!(
            "`{}` is damaged (its size or CRC-32 does not match)",
            entry.name
        );
    }
    Ok(())
}

/// Unpacks the archive `zip` into the folder `into` (made if missing), within `limits`: the files written, relative
/// to `into`. Nothing is written outside `into`; on an error, what was written stays for the caller to remove.
pub fn unpack(zip: &Path, into: &Path, limits: Limits) -> Result<Vec<PathBuf>> {
    let mut file = File::open(zip).with_context(|| format!("open {}", zip.display()))?;
    let entries = entries(&mut file, limits).with_context(|| format!("read {}", zip.display()))?;
    fs::create_dir_all(into)?;
    let mut written = Vec::new();
    for entry in &entries {
        let path = safe_path(&entry.name)?;
        let target = into.join(&path);
        if entry.name.ends_with('/') {
            fs::create_dir_all(&target)?;
            continue;
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        unpack_file(&mut file, entry, &target)?;
        written.push(path);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    /// A zip archive of `(name, bytes, deflated, unix mode)` entries.
    fn archive(entries: &[(&str, &[u8], bool, u32)]) -> Vec<u8> {
        let (mut out, mut central) = (Vec::new(), Vec::new());
        for (name, data, deflated, mode) in entries {
            let mut crc = flate2::Crc::new();
            crc.update(data);
            let stored = if *deflated {
                let mut e = flate2::write::DeflateEncoder::new(Vec::new(), Default::default());
                e.write_all(data).unwrap();
                e.finish().unwrap()
            } else {
                data.to_vec()
            };
            let (method, offset) = (
                if *deflated {
                    8u16
                } else {
                    0
                },
                out.len() as u32,
            );
            let common = |b: &mut Vec<u8>| {
                b.extend(method.to_le_bytes());
                b.extend([0; 4]);
                b.extend(crc.sum().to_le_bytes());
                b.extend((stored.len() as u32).to_le_bytes());
                b.extend((data.len() as u32).to_le_bytes());
                b.extend((name.len() as u16).to_le_bytes());
                b.extend(0u16.to_le_bytes());
            };
            out.extend(LOCAL.to_le_bytes());
            out.extend([20, 0, 0, 0]);
            common(&mut out);
            out.extend(name.as_bytes());
            out.extend(&stored);
            central.extend(CENTRAL.to_le_bytes());
            central.extend([20, 3, 20, 0, 0, 0]);
            common(&mut central);
            central.extend([0; 6]);
            central.extend((mode << 16).to_le_bytes());
            central.extend(offset.to_le_bytes());
            central.extend(name.as_bytes());
        }
        let at = out.len() as u32;
        out.extend(&central);
        out.extend(END.to_le_bytes());
        out.extend([0; 4]);
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((entries.len() as u16).to_le_bytes());
        out.extend((central.len() as u32).to_le_bytes());
        out.extend(at.to_le_bytes());
        out.extend([0; 2]);
        out
    }

    const LIMITS: Limits = Limits {
        entries: 10,
        bytes: 1 << 20,
    };

    fn unpack_bytes(test: &str, zip: &[u8], limits: Limits) -> (PathBuf, Result<Vec<PathBuf>>) {
        let dir =
            std::env::temp_dir().join(format!("render-lab-archive-{test}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("a.zip"), zip).unwrap();
        let result = unpack(&dir.join("a.zip"), &dir.join("out"), limits);
        (dir, result)
    }

    #[test]
    fn stored_and_deflated_files_unpack() {
        let zip = archive(&[
            ("p/", b"", false, 0o40755),
            ("p/a.txt", b"stored", false, 0o100644),
            ("p/b/c.txt", &[7; 5000], true, 0o100644),
        ]);
        let (dir, result) = unpack_bytes("ok", &zip, LIMITS);
        assert_eq!(result.unwrap().len(), 2);
        assert_eq!(fs::read(dir.join("out/p/a.txt")).unwrap(), b"stored");
        assert_eq!(fs::read(dir.join("out/p/b/c.txt")).unwrap(), vec![7; 5000]);
    }

    #[test]
    fn paths_out_of_the_folder_are_refused() {
        for name in ["../x", "p/../../x", "/etc/x", "C:/x", "p\\x", "p//x", "./x"] {
            assert!(safe_path(name).is_err(), "{name}");
            let (dir, result) =
                unpack_bytes("escape", &archive(&[(name, b"x", false, 0o100644)]), LIMITS);
            assert!(result.is_err(), "{name}");
            assert!(!dir.join("x").exists() && !dir.parent().unwrap().join("x").exists());
        }
        assert_eq!(
            safe_path("p/q/r.wav").unwrap(),
            Path::new("p").join("q").join("r.wav")
        );
    }

    #[test]
    fn links_damage_and_limits_are_refused() {
        let link = archive(&[("p/l", b"/etc/passwd", false, 0o120777)]);
        let refused = unpack_bytes("symlink", &link, LIMITS).1.unwrap_err();
        assert!(format!("{refused:#}").contains("`p/l` is a link"));
        let mut damaged = archive(&[("p/a", b"hello", false, 0o100644)]);
        damaged[30 + 3 + 1] ^= 1;
        assert!(
            format!("{:#}", unpack_bytes("crc", &damaged, LIMITS).1.unwrap_err())
                .contains("damaged")
        );
        let big = archive(&[("p/a", &[0; 100], true, 0o100644)]);
        let small = Limits {
            entries: 10,
            bytes: 99,
        };
        assert!(
            format!("{:#}", unpack_bytes("big", &big, small).1.unwrap_err()).contains("more than")
        );
        let many = Limits {
            entries: 0,
            bytes: 1 << 20,
        };
        assert!(
            format!("{:#}", unpack_bytes("many", &big, many).1.unwrap_err()).contains("entries")
        );
    }
}
