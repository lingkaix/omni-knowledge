//! Native compatibility metadata writes, explicitly NOT text/Git saved_local.
use crate::{
    bootstrap::{Bounds, Preflight},
    *,
};
use std::io::Write;
/// Establish/reuse the ONE native account for retained callers that have no earlier service startup.
/// The caller selects an actual external private cache parent; declarations never choose it.
pub fn start_resources(parent: &Path) -> Result<&'static Account, Error> {
    match Account::current() {
        Ok(a) => return Ok(a),
        Err(knowledge_core::admission::Error::NotStarted) => {}
        Err(e) => return Err(e.into()),
    }
    let parent = native::bind(parent)?;
    let dir = native::private_child(
        &parent,
        &format!("omni-native-{}", rustix::process::geteuid().as_raw()),
    )?;
    match Account::start(
        dir,
        knowledge_core::admission::Configuration {
            memory_bytes: None,
            storage_bytes: None,
        },
        knowledge_core::admission::Scheduling {
            max_pending: 128,
            max_records: 4096,
            max_descriptor_bytes: 4 * 1024 * 1024,
            per_mount: 1,
        },
    ) {
        Ok(a) => Ok(a),
        Err(knowledge_core::admission::Error::AlreadyStarted) => Ok(Account::current()?),
        Err(e) => Err(e.into()),
    }
}
/// Count exact serialized bytes without building an owned JSON tree/body.
pub struct JsonSize {
    /// Actual bytes written into this counting sink.
    pub bytes: u64,
    /// Finite native caller ceiling, not a format/corpus limit.
    pub limit: u64,
}
impl Write for JsonSize {
    /// Count before accepting bytes; overflow/excess never allocates an output body.
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(b.len() as u64)
            .filter(|n| *n <= self.limit)
            .ok_or_else(|| std::io::Error::other("JSON work budget"))?;
        Ok(b.len())
    }
    /// No external output exists.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
/// Stream bounded serialization directly to an exclusive candidate, without truncate/replace.
struct Bounded<'a> {
    /// Exact operation-owned native candidate.
    file: &'a mut File,
    /// Remaining admitted bytes.
    left: u64,
}
impl Write for Bounded<'_> {
    /// Refuse before a serialization chunk can exceed the caller's finite native plan.
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        if b.len() as u64 > self.left {
            return Err(std::io::Error::other("JSON work budget"));
        }
        self.file.write_all(b)?;
        self.left -= b.len() as u64;
        Ok(b.len())
    }
    /// All writes are direct and synchronized separately.
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<P: WritePolicy> Store<P> {
    /// Write explicitly approved retained metadata/internal state without pretending it is a Git save.
    /// Only the root marker or .omni compatibility state is eligible; current context and expected bytes apply.
    /// Native v2 authored text MUST use apply; this method cannot mutate raw originals or arbitrary text.
    /// Busy is entry-lock contention before any operation intent, candidate or payload effect; callers
    /// may retry only that class with the same operation/expected version and a finite outer deadline.
    /// Other failures can retain synchronized recovery and must never be blindly replayed.
    pub fn write_auxiliary_json<T: Serialize>(
        &self,
        operation: &str,
        path: &str,
        expected: Option<&Version>,
        value: &T,
        preflight: &Preflight<'_>,
    ) -> Result<Version, Error> {
        let b = Bounds::new(preflight)?;
        b.identities(operation, &self.library, &self.writer)?;
        b.check()?;
        let c = self.gate()?;
        if path != "omni-knowledge.json" && !path.starts_with(".omni/") {
            return Err(Error::Context);
        }
        if path.len() as u64 > b.control() / 8 {
            return Err(Error::Budget);
        }
        knowledge_core::data::RelativePath::new(path).map_err(|_| Error::Invalid)?;
        if expected.is_some_and(|v| !digest_valid(&v.digest)) {
            return Err(Error::Invalid);
        }
        if c.outside_scope
            .iter()
            .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
        {
            return Err(Error::Context);
        }
        if native::observe(&self.root, path, b.bytes, b.deadline)?.as_ref() != expected {
            return Err(Error::Conflict);
        }
        let derived = path.starts_with(".omni/indexes/") || path == ".omni/current.json";
        let external = if derived {
            let cache = Account::current()?.generated_directory()?;
            if native::within(&cache, &self.root)?
                || native::identity(&cache)?.0 != native::identity(&self.root)?.0
            {
                return Err(Error::Context);
            }
            let mut size = JsonSize {
                bytes: 0,
                limit: b.bytes,
            };
            serde_json::to_writer_pretty(&mut size, value).map_err(|_| Error::Budget)?;
            if preflight.holding.remaining()?.temporary_bytes
                < size.bytes.checked_add(1).ok_or(Error::Budget)?
            {
                return Err(Error::Budget);
            }
            Some(native::private_child(&cache, "auxiliary-v1")?)
        } else {
            None
        };
        let lane = self.lane()?;
        let recovery = external.as_ref().unwrap_or(&lane.recovery);
        let dir =
            native::new_child(recovery, &hash(operation.as_bytes()))?.ok_or(Error::Conflict)?;
        let fp = hash(
            serde_json::to_string(&(path, expected))
                .map_err(|_| Error::Invalid)?
                .as_bytes(),
        );
        let mut journal = journal::Journal::open(&lane.journal, &self.writer, b.control())?;
        journal.append(operation, &fp, None, "auxiliary_intent")?;
        native::control_new(&dir, "auxiliary.json", &(1u32, path, expected))?;
        let mut file = native::create_file(&dir, "new-0")?;
        serde_json::to_writer_pretty(
            Bounded {
                file: &mut file,
                left: b.bytes,
            },
            value,
        )
        .map_err(|_| Error::Budget)?;
        file.write_all(b"\n").map_err(|_| Error::Io)?;
        file.sync_all().map_err(|_| Error::Io)?;
        drop(file);
        dir.sync_all().map_err(|_| Error::Io)?;
        let candidate = native::version(
            native::regular(&dir, "new-0")?.ok_or(Error::Recovery)?,
            b.bytes,
            b.deadline,
        )?;
        self.live(&c)?;
        if native::observe(&self.root, path, b.bytes, b.deadline)?.as_ref() != expected {
            return Err(Error::Conflict);
        }
        let (parent, name) = native::parent(&self.root, path, true)?;
        if let Some(expected) = expected {
            native::displace(&parent, &name, &dir, "old-0")?;
            let actual = native::version(
                native::regular(&dir, "old-0")?.ok_or(Error::Recovery)?,
                b.bytes,
                b.deadline,
            )?;
            if !same_displaced(expected, &actual) {
                return Err(Error::Conflict);
            }
        }
        self.live(&c)?;
        native::install(&dir, "new-0", &parent, &name)?;
        let actual =
            native::observe(&self.root, path, b.bytes, b.deadline)?.ok_or(Error::Indeterminate)?;
        if actual.digest != candidate.digest {
            return Err(Error::Indeterminate);
        }
        self.live(&c)?;
        journal.append_observed(operation, &fp, path, &actual.digest, "metadata_installed")?;
        Ok(actual)
    }
    /// Observe retained auxiliary bytes through prepaid bounded native I/O, not a content permission grant.
    pub fn observe_auxiliary(
        &self,
        path: &str,
        preflight: &Preflight<'_>,
    ) -> Result<Option<Version>, Error> {
        let b = Bounds::new(preflight)?;
        let c = self.gate()?;
        if path != "omni-knowledge.json" && !path.starts_with(".omni/") {
            return Err(Error::Context);
        }
        if path.len() as u64 > b.control() / 8 {
            return Err(Error::Budget);
        }
        if c.outside_scope
            .iter()
            .any(|p| path == p || path.strip_prefix(p).is_some_and(|s| s.starts_with('/')))
        {
            return Err(Error::Context);
        }
        knowledge_core::data::RelativePath::new(path).map_err(|_| Error::Invalid)?;
        let observed = native::observe(&self.root, path, b.bytes, b.deadline)?;
        self.live(&c)?;
        Ok(observed)
    }
}
