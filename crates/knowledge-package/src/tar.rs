//! Bounded POSIX ustar/PAX carrier IO over held native files; no extraction or hook execution.
use crate::source::{BoundedBytes, Version};
use crate::{
    Error, RelativePath, Work, charge_fresh, gate, manifest, note_file, note_written, preallocate,
    source,
};
use std::{fs::File, io::Write};
use unicode_normalization::UnicodeNormalization;

/// Genuine retained-v1 carrier ceilings, independently narrowed by every caller quota.
const MAX_BYTES: u64 = 8_589_934_592;
/// The foreign carrier's finite extra allowance includes headers, padding, and PAX framing.
const FRAMING: u64 = 16_777_216;
/// OpenKit's maximum individual artifact length; not an application-wide corpus cap.
const MAX_FILE: u64 = 1_000_000_000;

/// A physical regular member or empty directory, bound to one checked container extent.
pub(crate) struct Member {
    pub(crate) path: RelativePath,
    pub(crate) offset: u64,
    pub(crate) length: u64,
    pub(crate) directory: bool,
}

/// Closed member table and immutable native version; only the original held descriptor is used.
pub(crate) struct Index {
    pub(crate) version: Version,
    pub(crate) members: std::collections::BTreeMap<String, Member>,
}

impl Index {
    /// Logarithmic exact member lookup with transient case-fold workspace funded before allocation.
    pub(crate) fn member(&self, path: &str, work: &Work<'_>) -> Result<&Member, Error> {
        gate(work)?;
        let _scratch = preallocate(work, path.len() as u64 * 3 + 256)?;
        let key = path.to_lowercase();
        let member = self
            .members
            .get(&key)
            .filter(|m| m.path.as_str() == path && !m.directory)
            .ok_or(Error::Invalid)?;
        gate(work)?;
        Ok(member)
    }

    /// Reject native mutation without reopening the archive's display pathname.
    pub(crate) fn revalidate(&self, file: &File, work: &mut Work<'_>) -> Result<(), Error> {
        note_file(work)?;
        if Version::of(&file.metadata().map_err(|_| Error::Io)?) != self.version {
            return Err(Error::Drift);
        }
        Ok(())
    }
}

/// Verify all framing before interpreting the v1 manifest, returning only its bounded bytes.
pub(crate) fn open(file: &File, work: &mut Work<'_>) -> Result<(Index, BoundedBytes), Error> {
    note_file(work)?;
    let metadata = file.metadata().map_err(|_| Error::Io)?;
    let length = metadata.len();
    if !metadata.is_file() || length < 1024 || length % 512 != 0 {
        return Err(Error::Invalid);
    }
    if length > work.bounds.quotas.max_archive_bytes || length > MAX_BYTES + FRAMING {
        return Err(Error::Quota);
    }
    let version = Version::of(&metadata);
    let _scratch = preallocate(work, 1024)?;
    let mut header = [0u8; 512];
    let mut members: std::collections::BTreeMap<String, Member> = std::collections::BTreeMap::new();
    let mut offset = 0u64;
    let mut total = 0u64;
    let mut pax: Option<BoundedBytes> = None;
    loop {
        gate(work)?;
        note_file(work)?;
        source::read_exact_at(file, offset, &mut header, work)?;
        if header.iter().all(|b| *b == 0) {
            if pax.is_some() || offset.checked_add(1024).ok_or(Error::Invalid)? > length {
                return Err(Error::Invalid);
            }
            let mut at = offset + 512;
            while at < length {
                source::read_exact_at(file, at, &mut header, work)?;
                if header.iter().any(|b| *b != 0) {
                    return Err(Error::Hostile);
                }
                at += 512;
            }
            break;
        }
        if offset.checked_add(512).ok_or(Error::Invalid)? > length {
            return Err(Error::Invalid);
        }
        let checksum = octal(&header[148..156])?;
        let actual: u64 = header
            .iter()
            .enumerate()
            .map(|(i, b)| {
                if (148..156).contains(&i) {
                    32
                } else {
                    u64::from(*b)
                }
            })
            .sum();
        if checksum != actual || &header[257..263] != b"ustar\0" || &header[263..265] != b"00" {
            return Err(Error::Invalid);
        }
        if header[157..257].iter().any(|b| *b != 0) {
            return Err(Error::Hostile);
        }
        let mut size = octal(&header[124..136])?;
        let data = offset.checked_add(512).ok_or(Error::Invalid)?;
        if header[156] == b'x' {
            if pax.is_some() || size > FRAMING || size > work.bounds.quotas.max_entry_expanded {
                return Err(Error::Quota);
            }
            checked_end(data, size, length)?;
            let mut bytes = BoundedBytes::reserve(size, work)?;
            bytes.bytes.resize(size as usize, 0);
            source::read_exact_at(file, data, &mut bytes.bytes, work)?;
            // Validate once now, before retaining arbitrary extension data.
            pax_fields(&bytes.bytes)?;
            pax = Some(bytes);
            offset = padded_end(data, size)?;
            continue;
        }
        if !matches!(header[156], 0 | b'0' | b'5') {
            return Err(Error::Hostile);
        }
        let fields = pax.as_ref().map(|p| pax_fields(&p.bytes)).transpose()?;
        if let Some(override_size) = fields.as_ref().and_then(|p| p.1) {
            size = override_size;
        }
        let directory = header[156] == b'5';
        if directory && size != 0 {
            return Err(Error::Hostile);
        }
        if size > MAX_FILE || size > work.bounds.quotas.max_entry_expanded {
            return Err(Error::Quota);
        }
        checked_end(data, size, length)?;
        total = total.checked_add(size).ok_or(Error::Quota)?;
        if total > MAX_BYTES || total > work.bounds.quotas.max_total_expanded {
            return Err(Error::Quota);
        }
        if members.len() as u64 >= work.bounds.quotas.max_entries.min(10_000) {
            return Err(Error::Quota);
        }
        let base = text(&header[..100])?;
        let prefix = text(&header[345..500])?;
        let raw = fields.as_ref().and_then(|p| p.0);
        let name_length = raw.map_or(
            base.len() + prefix.len() + usize::from(!prefix.is_empty()),
            str::len,
        );
        if name_length
            > work
                .bounds
                .quotas
                .max_path_bytes
                .saturating_add(usize::from(directory))
        {
            return Err(Error::Quota);
        }
        charge_fresh(work, name_length as u64 * 6 + 256, 0)?;
        let _name_scratch = preallocate(work, name_length as u64 * 6 + 512)?;
        let name = raw.map_or_else(
            || {
                if prefix.is_empty() {
                    base.to_owned()
                } else {
                    format!("{prefix}/{base}")
                }
            },
            str::to_owned,
        );
        let name = if directory {
            name.strip_suffix('/').unwrap_or(&name)
        } else {
            &name
        };
        confined(name, work)?;
        let folded = name.to_lowercase();
        if members.contains_key(&folded) {
            return Err(Error::Hostile);
        }
        for (at, _) in folded.match_indices('/') {
            gate(work)?;
            if members.get(&folded[..at]).is_some_and(|m| !m.directory) {
                return Err(Error::Hostile);
            }
        }
        if !directory {
            let prefix = format!("{folded}/");
            if members
                .range(prefix.clone()..)
                .next()
                .is_some_and(|(name, _)| name.starts_with(&prefix))
            {
                return Err(Error::Hostile);
            }
        }
        members.insert(
            folded,
            Member {
                path: RelativePath::new(name).map_err(|_| Error::Hostile)?,
                offset: data,
                length: size,
                directory,
            },
        );
        pax = None;
        offset = padded_end(data, size)?;
        if offset > length {
            return Err(Error::Invalid);
        }
    }
    let manifest = members
        .get("manifest.json")
        .filter(|e| e.path.as_str() == "manifest.json" && !e.directory)
        .ok_or(Error::Invalid)?;
    if manifest.length > 1_048_576 {
        return Err(Error::Quota);
    }
    let mut bytes = BoundedBytes::reserve(manifest.length, work)?;
    bytes.bytes.resize(manifest.length as usize, 0);
    source::read_exact_at(file, manifest.offset, &mut bytes.bytes, work)?;
    let index = Index { version, members };
    index.revalidate(file, work)?;
    Ok((index, bytes))
}

/// Validate canonical confined names, including aliases that differ only by Unicode or case.
pub(crate) fn confined(name: &str, work: &Work<'_>) -> Result<(), Error> {
    if name.len() > work.bounds.quotas.max_path_bytes
        || name.split('/').count() > work.bounds.quotas.max_depth
    {
        return Err(Error::Quota);
    }
    if !name.nfc().eq(name.chars()) || name.chars().any(char::is_control) {
        return Err(Error::Hostile);
    }
    manifest::path(name).map(|_| ())
}

/// Read a zero-terminated field without tolerating a hidden nonzero suffix or invalid UTF-8.
fn text(field: &[u8]) -> Result<&str, Error> {
    let end = field.iter().position(|b| *b == 0).unwrap_or(field.len());
    if field[end..].iter().any(|b| *b != 0) {
        return Err(Error::Invalid);
    }
    std::str::from_utf8(&field[..end]).map_err(|_| Error::Hostile)
}

/// Decode finite positive POSIX octal, rejecting GNU base-256 and overflow.
fn octal(field: &[u8]) -> Result<u64, Error> {
    let digits = field.trim_ascii();
    let end = digits.iter().position(|b| *b == 0).unwrap_or(digits.len());
    if digits[end..].iter().any(|b| *b != 0 && *b != b' ') {
        return Err(Error::Invalid);
    }
    let digits = digits[..end].trim_ascii();
    if digits.is_empty() {
        return Ok(0);
    }
    digits.iter().try_fold(0u64, |n, b| {
        if (b'0'..=b'7').contains(b) {
            n.checked_mul(8)
                .and_then(|n| n.checked_add(u64::from(b - b'0')))
                .ok_or(Error::Invalid)
        } else {
            Err(Error::Invalid)
        }
    })
}

/// Admit one exact payload extent before any allocation or native read.
fn checked_end(start: u64, size: u64, length: u64) -> Result<(), Error> {
    if padded_end(start, size)? > length {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// Checked 512-byte payload alignment, never a wrapping archive slice.
fn padded_end(start: u64, size: u64) -> Result<u64, Error> {
    start
        .checked_add(size.checked_add(511).ok_or(Error::Invalid)? / 512 * 512)
        .ok_or(Error::Invalid)
}

/// Parse local PAX length records without accepting links, sparse encodings, or duplicate control keys.
fn pax_fields(bytes: &[u8]) -> Result<(Option<&str>, Option<u64>), Error> {
    let mut at = 0;
    let mut path = None;
    let mut size = None;
    while at < bytes.len() {
        let space = bytes[at..]
            .iter()
            .position(|b| *b == b' ')
            .ok_or(Error::Invalid)?
            + at;
        let number = std::str::from_utf8(&bytes[at..space]).map_err(|_| Error::Invalid)?;
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            return Err(Error::Invalid);
        }
        let count = number.parse::<usize>().map_err(|_| Error::Invalid)?;
        let end = at
            .checked_add(count)
            .filter(|e| *e <= bytes.len() && *e > space + 1)
            .ok_or(Error::Invalid)?;
        if bytes[end - 1] != b'\n' {
            return Err(Error::Invalid);
        }
        let entry = std::str::from_utf8(&bytes[space + 1..end - 1]).map_err(|_| Error::Hostile)?;
        let (key, value) = entry.split_once('=').ok_or(Error::Invalid)?;
        if key.starts_with("GNU.sparse.")
            || key == "SCHILY.realsize"
            || key == "linkpath"
            || value.contains('\0')
        {
            return Err(Error::Hostile);
        }
        match key {
            "path" => {
                if path.replace(value).is_some() {
                    return Err(Error::Invalid);
                }
            }
            "size" => {
                if value.is_empty()
                    || !value.bytes().all(|b| b.is_ascii_digit())
                    || size
                        .replace(value.parse::<u64>().map_err(|_| Error::Invalid)?)
                        .is_some()
                {
                    return Err(Error::Invalid);
                }
            }
            "hdrcharset" if value != "ISO-IR 10646 2000 UTF-8" => return Err(Error::Unsupported),
            _ => {}
        }
        at = end;
    }
    Ok((path, size))
}

/// Write bytes only after output admission; reconcile short writes and exact generated extents.
pub(crate) fn write(
    output: &mut dyn Write,
    bytes: &[u8],
    work: &mut Work<'_>,
) -> Result<(), Error> {
    charge_fresh(work, 0, bytes.len() as u64)?;
    let mut at = 0;
    while at < bytes.len() {
        gate(work)?;
        let n = output.write(&bytes[at..]).map_err(|_| Error::Io)?;
        if n == 0 {
            return Err(Error::Io);
        }
        note_written(work, n as u64)?;
        at += n;
    }
    Ok(())
}

/// Emit one portable regular-member header, using a real PAX path for long UTF-8 names.
pub(crate) fn header(
    output: &mut File,
    path: &str,
    size: u64,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    confined(path, work)?;
    if size > MAX_FILE || size > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    let _scratch = preallocate(work, path.len() as u64 * 3 + 2048)?;
    if path.len() > 100 {
        let body = format!("path={path}\n");
        let mut length = body.len() + 2;
        loop {
            let next = body.len() + length.to_string().len() + 1;
            if next == length {
                break;
            }
            length = next;
        }
        let record = format!("{length} {body}");
        raw_header(output, "PaxHeader", record.len() as u64, b'x', work)?;
        write(output, record.as_bytes(), work)?;
        padding(output, record.len() as u64, work)?;
        raw_header(output, "PaxMember", size, b'0', work)
    } else {
        raw_header(output, path, size, b'0', work)
    }
}

/// Write deterministic ustar metadata; no execution, ownership, or special-file bits travel.
fn raw_header(
    output: &mut File,
    path: &str,
    size: u64,
    kind: u8,
    work: &mut Work<'_>,
) -> Result<(), Error> {
    let mut bytes = [0u8; 512];
    bytes[..path.len()].copy_from_slice(path.as_bytes());
    bytes[100..108].copy_from_slice(b"0000600\0");
    bytes[108..116].copy_from_slice(b"0000000\0");
    bytes[116..124].copy_from_slice(b"0000000\0");
    let size = format!("{size:011o}\0");
    if size.len() != 12 {
        return Err(Error::Quota);
    }
    bytes[124..136].copy_from_slice(size.as_bytes());
    bytes[136..148].copy_from_slice(b"00000000000\0");
    bytes[148..156].fill(b' ');
    bytes[156] = kind;
    bytes[257..263].copy_from_slice(b"ustar\0");
    bytes[263..265].copy_from_slice(b"00");
    let checksum: u64 = bytes.iter().map(|b| u64::from(*b)).sum();
    let checksum = format!("{checksum:06o}\0 ");
    bytes[148..156].copy_from_slice(checksum.as_bytes());
    write(output, &bytes, work)
}

/// Pad one written payload without creating a whole-carrier buffer.
pub(crate) fn padding(output: &mut File, size: u64, work: &mut Work<'_>) -> Result<(), Error> {
    let padding = (512 - size % 512) % 512;
    write(output, &[0u8; 512][..padding as usize], work)
}
