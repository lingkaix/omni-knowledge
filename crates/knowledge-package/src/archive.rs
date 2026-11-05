//! Bounded ZIP framing. Declared sizes never grant allocation or output.
use crate::manifest::forbidden;
use crate::{Error, Method, Work, charge_fresh, note_written, preallocate};
use crc32fast::Hasher;
use flate2::{Decompress, FlushDecompress, Status};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::Instant;
use unicode_normalization::UnicodeNormalization;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

const SCRATCH: usize = 8192;
const WINDOW: u64 = 64 * 1024;
const STORED: u16 = 0;
const DEFLATE: u16 = 8;
const S_IFMT: u32 = 0o170000;
const S_IFREG: u32 = 0o100000;
const S_IFDIR: u32 = 0o040000;

#[derive(Clone, Debug)]
pub(crate) struct Member {
    pub name: String,
    pub method: u16,
    pub crc: u32,
    pub compressed: u64,
    pub uncompressed: u64,
    pub local_offset: u64,
    pub data_offset: u64,
    pub directory: bool,
}

pub(crate) fn list(file: &mut File, work: &mut Work<'_>) -> Result<Vec<Member>, Error> {
    let len = file.metadata().map_err(|_| Error::Io)?.len();
    if len > work.bounds.quotas.max_archive_bytes {
        return Err(Error::Quota);
    }
    let eocd = read_eocd(file, len, work)?;
    if eocd.entries > work.bounds.quotas.max_entries {
        return Err(Error::Quota);
    }
    note_file(work)?;
    let mut members = Vec::new();
    let mut cursor = eocd.cd_offset;
    let cd_end = eocd
        .cd_offset
        .checked_add(eocd.cd_size)
        .ok_or(Error::Hostile)?;
    for _ in 0..eocd.entries {
        let member = read_central(file, work, &mut cursor)?;
        members.push(member);
    }
    if cursor != cd_end {
        return Err(Error::Disagreement);
    }
    for member in &mut members {
        bind_local(file, work, member, eocd.cd_offset)?;
    }
    reject_layout(&members, eocd.cd_offset, work)?;
    Ok(members)
}

pub(crate) enum Out<'a> {
    /// Hash and drop the actual output.
    Drop,
    /// Retain actual output after charging memory.
    Keep(&'a mut Vec<u8>),
    /// Write actual output after charging generated storage.
    Disk(&'a mut File),
}

pub(crate) fn stream(
    file: &mut File,
    member: &Member,
    work: &mut Work<'_>,
    out: &mut Out<'_>,
) -> Result<String, Error> {
    if member.directory {
        return Err(Error::Hostile);
    }
    if member.uncompressed > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    if member.compressed > work.bounds.quotas.max_archive_bytes {
        return Err(Error::Quota);
    }
    note_file(work)?;
    let mut sha = Sha256::new();
    let mut crc = Hasher::new();
    let mut produced = 0u64;
    match member.method {
        STORED => stream_stored(file, member, work, &mut produced, &mut sha, &mut crc, out)?,
        DEFLATE => stream_deflate(file, member, work, &mut produced, &mut sha, &mut crc, out)?,
        _ => return Err(Error::Unsupported),
    }
    if produced != member.uncompressed {
        return Err(Error::Disagreement);
    }
    if crc.finalize() != member.crc {
        return Err(Error::Checksum);
    }
    work.expanded = work.expanded.checked_add(produced).ok_or(Error::Quota)?;
    if work.expanded > work.bounds.quotas.max_total_expanded {
        return Err(Error::Quota);
    }
    Ok(hex(&sha.finalize()))
}

pub(crate) fn write_zip(file: File, parts: &[(&str, Method, &[u8])]) -> Result<File, Error> {
    let mut zip = ZipWriter::new(file);
    for (name, method, bytes) in parts {
        let options = SimpleFileOptions::default().compression_method(match method {
            Method::Stored => CompressionMethod::Stored,
            Method::Deflate => CompressionMethod::Deflated,
        });
        zip.start_file(*name, options).map_err(|_| Error::Io)?;
        zip.write_all(bytes).map_err(|_| Error::Io)?;
    }
    zip.finish().map_err(|_| Error::Io)
}

fn stream_stored(
    file: &mut File,
    member: &Member,
    work: &mut Work<'_>,
    produced: &mut u64,
    sha: &mut Sha256,
    crc: &mut Hasher,
    out: &mut Out<'_>,
) -> Result<(), Error> {
    if member.compressed != member.uncompressed {
        return Err(Error::Disagreement);
    }
    let _scratch = preallocate(work, SCRATCH as u64)?;
    let mut buf = [0u8; SCRATCH];
    let mut left = member.compressed;
    let mut pos = member.data_offset;
    while left > 0 {
        gate(work)?;
        let n = usize::try_from(left.min(SCRATCH as u64)).map_err(|_| Error::Hostile)?;
        read_exact_at(file, work, pos, &mut buf[..n])?;
        accept(work, member.compressed, *produced, n as u64)?;
        sha.update(&buf[..n]);
        crc.update(&buf[..n]);
        deliver(work, out, &buf[..n])?;
        *produced += n as u64;
        pos += n as u64;
        left -= n as u64;
    }
    Ok(())
}

fn stream_deflate(
    file: &mut File,
    member: &Member,
    work: &mut Work<'_>,
    produced: &mut u64,
    sha: &mut Sha256,
    crc: &mut Hasher,
    out: &mut Out<'_>,
) -> Result<(), Error> {
    let _scratch = preallocate(work, WINDOW + (SCRATCH as u64) * 2)?;
    let mut dec = Decompress::new(false);
    let mut input = [0u8; SCRATCH];
    let mut output = [0u8; SCRATCH];
    let mut filled = 0usize;
    let mut used = 0usize;
    let mut left = member.compressed;
    let mut pos = member.data_offset;
    loop {
        gate(work)?;
        if used == filled && left > 0 {
            let n = usize::try_from(left.min(SCRATCH as u64)).map_err(|_| Error::Hostile)?;
            read_exact_at(file, work, pos, &mut input[..n])?;
            pos += n as u64;
            left -= n as u64;
            filled = n;
            used = 0;
        }
        let before_in = dec.total_in();
        let before_out = dec.total_out();
        let flush = if left == 0 && used == filled {
            FlushDecompress::Finish
        } else {
            FlushDecompress::None
        };
        let status = dec
            .decompress(&input[used..filled], &mut output, flush)
            .map_err(|_| Error::Hostile)?;
        used += usize::try_from(dec.total_in() - before_in).map_err(|_| Error::Hostile)?;
        let count = usize::try_from(dec.total_out() - before_out).map_err(|_| Error::Hostile)?;
        if count > 0 {
            accept(work, member.compressed, *produced, count as u64)?;
            sha.update(&output[..count]);
            crc.update(&output[..count]);
            deliver(work, out, &output[..count])?;
            *produced += count as u64;
        }
        if status == Status::StreamEnd {
            break;
        }
        if count == 0
            && dec.total_in() == before_in
            && (left == 0 && used == filled || used < filled)
        {
            return Err(Error::Hostile);
        }
    }
    if dec.total_in() != member.compressed || dec.total_out() != *produced {
        return Err(Error::Disagreement);
    }
    Ok(())
}

fn deliver(work: &mut Work<'_>, out: &mut Out<'_>, chunk: &[u8]) -> Result<(), Error> {
    match out {
        Out::Drop => Ok(()),
        Out::Keep(buffer) => {
            charge_fresh(work, chunk.len() as u64, 0)?;
            buffer.extend_from_slice(chunk);
            Ok(())
        }
        Out::Disk(file) => {
            account(work, 0, chunk.len() as u64)?;
            file.write_all(chunk).map_err(|_| Error::Io)?;
            note_written(work, chunk.len() as u64)?;
            Ok(())
        }
    }
}

fn accept(work: &mut Work<'_>, compressed: u64, produced: u64, adding: u64) -> Result<(), Error> {
    let next = produced.checked_add(adding).ok_or(Error::Quota)?;
    if next > work.bounds.quotas.max_entry_expanded
        || work.expanded.checked_add(next).ok_or(Error::Quota)?
            > work.bounds.quotas.max_total_expanded
    {
        return Err(Error::Quota);
    }
    let base = compressed.max(1);
    if next / base > work.bounds.quotas.max_ratio {
        return Err(Error::Ratio);
    }
    Ok(())
}

struct Eocd {
    entries: u64,
    cd_size: u64,
    cd_offset: u64,
}

fn read_eocd(file: &mut File, len: u64, work: &mut Work<'_>) -> Result<Eocd, Error> {
    if len < 22 {
        return Err(Error::Truncated);
    }
    let window = len.min(22 + 65_535);
    let _scratch = preallocate(work, window)?;
    let mut buf = vec![0u8; usize::try_from(window).map_err(|_| Error::Hostile)?];
    read_exact_at(file, work, len - window, &mut buf)?;
    let mut found = None;
    if buf.len() >= 22 {
        let mut index = buf.len() - 22;
        loop {
            if buf[index..].starts_with(&0x06054b50u32.to_le_bytes()) {
                let comment = u16_at(&buf, index + 20)? as usize;
                if index + 22 + comment == buf.len() {
                    found = Some(index);
                    break;
                }
            }
            if index == 0 {
                break;
            }
            index -= 1;
        }
    }
    let index = found.ok_or(Error::Truncated)?;
    let disk = u16_at(&buf, index + 4)?;
    let cd_disk = u16_at(&buf, index + 6)?;
    let entries_disk = u16_at(&buf, index + 8)?;
    let entries = u16_at(&buf, index + 10)?;
    let cd_size = u32_at(&buf, index + 12)?;
    let cd_offset = u32_at(&buf, index + 16)?;
    if disk != 0 || cd_disk != 0 {
        return Err(Error::Hostile);
    }
    let eocd_pos = len - window + index as u64;
    let need_zip64 = entries == u16::MAX
        || entries_disk == u16::MAX
        || cd_size == u32::MAX
        || cd_offset == u32::MAX;
    if !need_zip64 {
        if entries_disk != entries {
            return Err(Error::Disagreement);
        }
        return Ok(Eocd {
            entries: u64::from(entries),
            cd_size: u64::from(cd_size),
            cd_offset: u64::from(cd_offset),
        });
    }
    if index < 20 {
        return Err(Error::Hostile);
    }
    let locator = index - 20;
    if u32_at(&buf, locator)? != 0x07064b50 {
        return Err(Error::Hostile);
    }
    if u32_at(&buf, locator + 4)? != 0 || u32_at(&buf, locator + 16)? != 1 {
        return Err(Error::Hostile);
    }
    let zip64_at = u64_at(&buf, locator + 8)?;
    if zip64_at >= eocd_pos || eocd_pos - zip64_at > window {
        return read_zip64(file, work, zip64_at, eocd_pos);
    }
    let local = usize::try_from(zip64_at - (len - window)).map_err(|_| Error::Hostile)?;
    parse_zip64(&buf, local, eocd_pos)
}

fn read_zip64(file: &mut File, work: &mut Work<'_>, at: u64, eocd_pos: u64) -> Result<Eocd, Error> {
    let _scratch = preallocate(work, 56)?;
    let mut buf = [0u8; 56];
    read_exact_at(file, work, at, &mut buf)?;
    parse_zip64(&buf, 0, eocd_pos)
}

fn parse_zip64(buf: &[u8], at: usize, eocd_pos: u64) -> Result<Eocd, Error> {
    if u32_at(buf, at)? != 0x06064b50 {
        return Err(Error::Hostile);
    }
    if u32_at(buf, at + 16)? != 0 || u32_at(buf, at + 20)? != 0 {
        return Err(Error::Hostile);
    }
    let entries_disk = u64_at(buf, at + 24)?;
    let entries = u64_at(buf, at + 32)?;
    let cd_size = u64_at(buf, at + 40)?;
    let cd_offset = u64_at(buf, at + 48)?;
    if entries_disk != entries || cd_offset.checked_add(cd_size).ok_or(Error::Hostile)? > eocd_pos {
        return Err(Error::Disagreement);
    }
    Ok(Eocd {
        entries,
        cd_size,
        cd_offset,
    })
}

fn read_central(file: &mut File, work: &mut Work<'_>, cursor: &mut u64) -> Result<Member, Error> {
    let _head_scratch = preallocate(work, 46)?;
    let mut head = [0u8; 46];
    read_exact_at(file, work, *cursor, &mut head)?;
    if u32_at(&head, 0)? != 0x02014b50 {
        return Err(Error::Hostile);
    }
    let made_by = u16_at(&head, 4)?;
    let flags = u16_at(&head, 8)?;
    let method = u16_at(&head, 10)?;
    let crc = u32_at(&head, 16)?;
    let comp32 = u32_at(&head, 20)?;
    let uncomp32 = u32_at(&head, 24)?;
    let name_len = u16_at(&head, 28)? as usize;
    let extra_len = u16_at(&head, 30)? as usize;
    let comment_len = u16_at(&head, 32)? as usize;
    let disk = u16_at(&head, 34)?;
    let external = u32_at(&head, 38)?;
    let offset32 = u32_at(&head, 42)?;
    if flags & 1 != 0 {
        return Err(Error::Hostile);
    }
    if flags & 0x8 != 0 {
        return Err(Error::Unsupported);
    }
    if disk != 0 && disk != u16::MAX {
        return Err(Error::Hostile);
    }
    if name_len == 0 || name_len > work.bounds.quotas.max_path_bytes {
        return Err(Error::Hostile);
    }
    charge_fresh(work, name_len as u64, 0)?;
    let _extra_scratch = preallocate(work, extra_len as u64)?;
    let mut name = vec![0u8; name_len];
    let mut extra = vec![0u8; extra_len];
    read_exact_at(
        file,
        work,
        cursor.checked_add(46).ok_or(Error::Hostile)?,
        &mut name,
    )?;
    read_exact_at(
        file,
        work,
        cursor
            .checked_add(46 + name_len as u64)
            .ok_or(Error::Hostile)?,
        &mut extra,
    )?;
    if comment_len > 0 {
        let _comment_scratch = preallocate(work, comment_len as u64)?;
        let mut ignore = vec![0u8; comment_len];
        read_exact_at(
            file,
            work,
            cursor
                .checked_add(46 + name_len as u64 + extra_len as u64)
                .ok_or(Error::Hostile)?,
            &mut ignore,
        )?;
    }
    *cursor = cursor
        .checked_add(46 + name_len as u64 + extra_len as u64 + comment_len as u64)
        .ok_or(Error::Hostile)?;
    let (compressed, uncompressed, local_offset) =
        zip64_fields(&extra, comp32, uncomp32, offset32, disk)?;
    let name = normalize_name(&name, flags, work)?;
    let directory = name.ends_with('/');
    if directory && (compressed != 0 || uncompressed != 0 || method != STORED) {
        return Err(Error::Hostile);
    }
    classify_mode(made_by, external, directory)?;
    if !directory && method != STORED && method != DEFLATE {
        return Err(Error::Unsupported);
    }
    Ok(Member {
        name,
        method,
        crc,
        compressed,
        uncompressed,
        local_offset,
        data_offset: 0,
        directory,
    })
}

fn bind_local(
    file: &mut File,
    work: &mut Work<'_>,
    member: &mut Member,
    cd_offset: u64,
) -> Result<(), Error> {
    let _head_scratch = preallocate(work, 30)?;
    let mut head = [0u8; 30];
    read_exact_at(file, work, member.local_offset, &mut head)?;
    if u32_at(&head, 0)? != 0x04034b50 {
        return Err(Error::Hostile);
    }
    let flags = u16_at(&head, 6)?;
    let method = u16_at(&head, 8)?;
    let crc = u32_at(&head, 14)?;
    let comp32 = u32_at(&head, 18)?;
    let uncomp32 = u32_at(&head, 22)?;
    let name_len = u16_at(&head, 26)? as usize;
    let extra_len = u16_at(&head, 28)? as usize;
    if flags & 1 != 0 || flags & 0x8 != 0 {
        return Err(Error::Hostile);
    }
    if name_len > work.bounds.quotas.max_path_bytes {
        return Err(Error::Hostile);
    }
    let _name_scratch = preallocate(work, (name_len + extra_len) as u64)?;
    let mut name = vec![0u8; name_len];
    let mut extra = vec![0u8; extra_len];
    let name_at = member.local_offset.checked_add(30).ok_or(Error::Hostile)?;
    read_exact_at(file, work, name_at, &mut name)?;
    read_exact_at(
        file,
        work,
        name_at.checked_add(name_len as u64).ok_or(Error::Hostile)?,
        &mut extra,
    )?;
    let local_name = normalize_name(&name, flags, work)?;
    if local_name != member.name || method != member.method || crc != member.crc {
        return Err(Error::Disagreement);
    }
    let (compressed, uncompressed, _) = zip64_fields(&extra, comp32, uncomp32, 0, 0)?;
    if compressed != member.compressed || uncompressed != member.uncompressed {
        return Err(Error::Disagreement);
    }
    let data_offset = name_at
        .checked_add(name_len as u64 + extra_len as u64)
        .ok_or(Error::Hostile)?;
    let end = data_offset
        .checked_add(member.compressed)
        .ok_or(Error::Hostile)?;
    if data_offset < member.local_offset || end > cd_offset {
        return Err(Error::Hostile);
    }
    member.data_offset = data_offset;
    Ok(())
}

fn reject_layout(members: &[Member], cd_offset: u64, work: &Work<'_>) -> Result<(), Error> {
    let mut names = Vec::new();
    let mut spans = Vec::new();
    for member in members {
        let depth = member.name.trim_end_matches('/').split('/').count();
        if depth > work.bounds.quotas.max_depth {
            return Err(Error::Hostile);
        }
        let mut folded = member.name.trim_end_matches('/').to_lowercase();
        if member.directory {
            folded.push('/');
        }
        names.push(folded);
        let end = member
            .data_offset
            .checked_add(member.compressed)
            .ok_or(Error::Hostile)?;
        if member.local_offset >= member.data_offset || end > cd_offset {
            return Err(Error::Hostile);
        }
        spans.push((member.local_offset, end));
    }
    names.sort();
    for index in 0..names.len() {
        for next in index + 1..names.len() {
            let left = names[index].trim_end_matches('/');
            let right = names[next].trim_end_matches('/');
            if !right.starts_with(left) {
                break;
            }
            let same = right.len() == left.len();
            let child = right.len() > left.len() && right.as_bytes()[left.len()] == b'/';
            if same || (child && !names[index].ends_with('/')) {
                return Err(Error::Hostile);
            }
            if !child && !same {
                break;
            }
        }
    }
    spans.sort_unstable();
    for pair in spans.windows(2) {
        if pair[0].1 > pair[1].0 {
            return Err(Error::Hostile);
        }
    }
    let _ = cd_offset;
    Ok(())
}

fn normalize_name(bytes: &[u8], flags: u16, work: &Work<'_>) -> Result<String, Error> {
    if flags & (1 << 11) == 0 && bytes.iter().any(|byte| !byte.is_ascii()) {
        return Err(Error::Hostile);
    }
    let name = std::str::from_utf8(bytes).map_err(|_| Error::Hostile)?;
    if name.nfc().collect::<String>() != name || name.len() > work.bounds.quotas.max_path_bytes {
        return Err(Error::Hostile);
    }
    let logical = name.trim_end_matches('/');
    if logical.is_empty()
        || logical
            .split('/')
            .any(|segment| segment.is_empty() || segment == "." || segment == "..")
        || name.starts_with('/')
        || name.contains('\\')
        || name.chars().any(|char| char.is_control())
    {
        return Err(Error::Hostile);
    }
    if forbidden(logical) {
        return Err(Error::Forbidden);
    }
    Ok(name.to_owned())
}

fn classify_mode(made_by: u16, external: u32, directory: bool) -> Result<(), Error> {
    if made_by >> 8 != 3 {
        return Ok(());
    }
    let kind = (external >> 16) & S_IFMT;
    if kind == 0 || kind == S_IFREG || (directory && kind == S_IFDIR) {
        return Ok(());
    }
    Err(Error::Hostile)
}

fn zip64_fields(
    extra: &[u8],
    comp32: u32,
    uncomp32: u32,
    offset32: u32,
    disk: u16,
) -> Result<(u64, u64, u64), Error> {
    let mut compressed = u64::from(comp32);
    let mut uncompressed = u64::from(uncomp32);
    let mut offset = u64::from(offset32);
    let mut cursor = 0;
    while cursor + 4 <= extra.len() {
        let id = u16::from_le_bytes(
            extra[cursor..cursor + 2]
                .try_into()
                .map_err(|_| Error::Hostile)?,
        );
        let size = u16::from_le_bytes(
            extra[cursor + 2..cursor + 4]
                .try_into()
                .map_err(|_| Error::Hostile)?,
        ) as usize;
        let body = extra
            .get(cursor + 4..cursor + 4 + size)
            .ok_or(Error::Hostile)?;
        if id == 0x0001 {
            let mut at = 0;
            if uncomp32 == u32::MAX {
                uncompressed = take_u64(body, &mut at)?;
            }
            if comp32 == u32::MAX {
                compressed = take_u64(body, &mut at)?;
            }
            if offset32 == u32::MAX {
                offset = take_u64(body, &mut at)?;
            }
            if disk == u16::MAX {
                let _disk64 = take_u32(body, &mut at)?;
            }
            if at != body.len() {
                return Err(Error::Disagreement);
            }
        } else if id == 0x7075 {
            return Err(Error::Hostile);
        }
        cursor += 4 + size;
    }
    if cursor != extra.len() {
        return Err(Error::Hostile);
    }
    Ok((compressed, uncompressed, offset))
}

fn take_u64(body: &[u8], at: &mut usize) -> Result<u64, Error> {
    let next = body.get(*at..*at + 8).ok_or(Error::Hostile)?;
    *at += 8;
    Ok(u64::from_le_bytes(
        next.try_into().map_err(|_| Error::Hostile)?,
    ))
}

fn take_u32(body: &[u8], at: &mut usize) -> Result<u32, Error> {
    let next = body.get(*at..*at + 4).ok_or(Error::Hostile)?;
    *at += 4;
    Ok(u32::from_le_bytes(
        next.try_into().map_err(|_| Error::Hostile)?,
    ))
}

fn read_exact_at(
    file: &mut File,
    work: &mut Work<'_>,
    pos: u64,
    buf: &mut [u8],
) -> Result<(), Error> {
    let len = buf.len() as u64;
    if work.read_bytes.checked_add(len).ok_or(Error::Quota)? > work.bounds.budget.max_bytes {
        return Err(Error::Quota);
    }
    file.seek(SeekFrom::Start(pos)).map_err(|_| Error::Io)?;
    file.read_exact(buf).map_err(|_| Error::Truncated)?;
    work.read_bytes += len;
    Ok(())
}

fn account(work: &mut Work<'_>, memory: u64, temporary: u64) -> Result<(), Error> {
    gate(work)?;
    charge_fresh(work, memory, temporary)
}

fn gate(work: &Work<'_>) -> Result<(), Error> {
    if work.cancel.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(Error::Cancelled);
    }
    if Instant::now() >= work.deadline {
        return Err(Error::Deadline);
    }
    Ok(())
}

fn note_file(work: &mut Work<'_>) -> Result<(), Error> {
    work.files = work.files.checked_add(1).ok_or(Error::Quota)?;
    if work.files > work.bounds.budget.max_files {
        return Err(Error::Quota);
    }
    Ok(())
}

fn u16_at(buf: &[u8], at: usize) -> Result<u16, Error> {
    Ok(u16::from_le_bytes(
        buf.get(at..at + 2)
            .ok_or(Error::Truncated)?
            .try_into()
            .map_err(|_| Error::Hostile)?,
    ))
}
fn u32_at(buf: &[u8], at: usize) -> Result<u32, Error> {
    Ok(u32::from_le_bytes(
        buf.get(at..at + 4)
            .ok_or(Error::Truncated)?
            .try_into()
            .map_err(|_| Error::Hostile)?,
    ))
}
fn u64_at(buf: &[u8], at: usize) -> Result<u64, Error> {
    Ok(u64::from_le_bytes(
        buf.get(at..at + 8)
            .ok_or(Error::Truncated)?
            .try_into()
            .map_err(|_| Error::Hostile)?,
    ))
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0xf) as usize] as char);
    }
    out
}

pub(crate) fn method_of(member: &Member) -> Result<Method, Error> {
    match member.method {
        STORED => Ok(Method::Stored),
        DEFLATE => Ok(Method::Deflate),
        _ => Err(Error::Unsupported),
    }
}
