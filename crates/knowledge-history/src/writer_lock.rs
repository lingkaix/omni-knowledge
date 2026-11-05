//! An exclusive flock is owned by the acquiring process, not every fork-inherited descriptor.

use fs2::FileExt;
use std::fs::File;

/// Actual writer file description; an unknown native worker may explicitly retain it instead of dropping it.
pub(crate) struct OwnedWriterLock {
    /// The original acquiring process alone may explicitly unlock the shared open-file description.
    owner_pid: u32,
    /// Taken only when ownership moves to an unresolved native worker.
    file: Option<File>,
}
impl OwnedWriterLock {
    /// Wrap only an already successfully acquired exclusive lock.
    pub(crate) fn new(file: File) -> Self {
        Self {
            owner_pid: std::process::id(),
            file: Some(file),
        }
    }
    /// Fork inheritance is not permission to unlock the original owner's lock.
    pub(crate) fn is_original_owner(&self) -> bool {
        self.owner_pid == std::process::id()
    }
    /// Transfer the actual descriptor to the native unresolved owner, without unlocking it.
    #[cfg(test)]
    pub(crate) fn into_file(mut self) -> File {
        self.file.take().expect("owned writer descriptor")
    }
}
impl Drop for OwnedWriterLock {
    /// Close-only flock release can be prolonged by an unrelated child; explicitly unlock only in the original owner.
    fn drop(&mut self) {
        if self.is_original_owner()
            && let Some(file) = &self.file
        {
            let _ = FileExt::unlock(file);
        }
    }
}

#[cfg(test)]
#[allow(unsafe_code)] // Only native dup2/FD ownership transfer around Command exec; never raw fork or Rust callbacks after fork.
mod tests {
    use super::*;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::net::UnixStream;
    use std::os::unix::process::CommandExt;
    use std::process::{Child, Command, Stdio};
    use std::time::{Duration, Instant};

    /// Finite real exec helper with a deliberately inherited copy of the actual writer open-file description.
    struct Helper {
        child: Child,
        channel: UnixStream,
    }
    impl Helper {
        fn start(file: &File, mode: &str, owner_pid: u32) -> Self {
            let (mut channel, inherited) = UnixStream::pair().unwrap();
            channel
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            channel
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let writer_fd = file.as_raw_fd();
            let channel_fd = inherited.as_raw_fd();
            // Reserve targets before spawn, so child dup2 cannot overwrite std's exec-error pipe or another source FD.
            // SAFETY: fcntl creates new owned descriptors; each successful descriptor is adopted exactly once below.
            let (writer_copy, channel_copy) = unsafe {
                let a = libc::fcntl(writer_fd, libc::F_DUPFD_CLOEXEC, 64);
                assert!(a >= 0);
                let a = File::from_raw_fd(a);
                let b = libc::fcntl(channel_fd, libc::F_DUPFD_CLOEXEC, a.as_raw_fd() + 1);
                assert!(b >= 0);
                (a, File::from_raw_fd(b))
            };
            let writer_target = writer_copy.as_raw_fd();
            let channel_target = channel_copy.as_raw_fd();
            let mut command = Command::new(std::env::current_exe().unwrap());
            command
                .env_clear()
                .env("PATH", "/usr/bin:/bin")
                .env("OMNI_OWNED_FLOCK_HELPER", mode)
                .env("OMNI_OWNED_FLOCK_PARENT", owner_pid.to_string())
                .env("OMNI_OWNED_FLOCK_FD", writer_target.to_string())
                .env("OMNI_OWNED_FLOCK_CHANNEL", channel_target.to_string())
                .args([
                    "--exact",
                    "writer_lock::tests::native_inherited_descriptor_lifetime",
                    "--test-threads=1",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            // SAFETY: only async-signal-safe dup2 with live owned, disjoint, pre-reserved FD targets; immediately followed by exec.
            unsafe {
                command.pre_exec(move || {
                    if libc::dup2(writer_fd, writer_target) < 0
                        || libc::dup2(channel_fd, channel_target) < 0
                    {
                        return Err(std::io::Error::last_os_error());
                    }
                    Ok(())
                });
            }
            let child = command.spawn().unwrap();
            drop(writer_copy);
            drop(channel_copy);
            drop(inherited);
            let mut ready = [0u8; 4];
            channel.read_exact(&mut ready).unwrap();
            assert_eq!(&ready, b"HELD");
            Self { child, channel }
        }
        fn finish(mut self) {
            self.channel.write_all(b"DROP").unwrap();
            let mut done = [0u8; 4];
            self.channel.read_exact(&mut done).unwrap();
            assert_eq!(&done, b"DONE");
            let end = Instant::now() + Duration::from_secs(2);
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert!(status.success());
                    break;
                }
                assert!(Instant::now() < end, "owned FD helper reap deadline");
                std::thread::sleep(Duration::from_millis(2));
            }
        }
    }
    impl Drop for Helper {
        /// The actual unreaped Child pins this PID; never rediscover or signal another process from serialized PID data.
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            let _ = self.child.kill();
            let end = Instant::now() + Duration::from_secs(2);
            while Instant::now() < end {
                if self.child.try_wait().ok().flatten().is_some() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(2));
            }
            panic!("owned inherited-FD helper collection unknown");
        }
    }

    /// The exec branch is folded into the real regression, not an ignored/unstarted extra test.
    fn helper(mode: &str) {
        let fd: i32 = std::env::var("OMNI_OWNED_FLOCK_FD")
            .unwrap()
            .parse()
            .unwrap();
        let socket: i32 = std::env::var("OMNI_OWNED_FLOCK_CHANNEL")
            .unwrap()
            .parse()
            .unwrap();
        let owner_pid: u32 = std::env::var("OMNI_OWNED_FLOCK_PARENT")
            .unwrap()
            .parse()
            .unwrap();
        // SAFETY: exactly these two descriptors were transferred across exec by the parent's native dup2.
        let (file, mut channel) =
            unsafe { (File::from_raw_fd(fd), UnixStream::from_raw_fd(socket)) };
        channel
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        channel
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let file = if mode == "child-guard-drop" {
            assert_ne!(owner_pid, std::process::id());
            // This is a genuine exec child with a genuine inherited OFD. It must not unlock the parent's still-live owner.
            drop(OwnedWriterLock {
                owner_pid,
                file: Some(file),
            });
            None
        } else {
            Some(file)
        };
        channel.write_all(b"HELD").unwrap();
        let mut command = [0u8; 4];
        channel.read_exact(&mut command).unwrap();
        assert_eq!(&command, b"DROP");
        drop(file);
        channel.write_all(b"DONE").unwrap();
    }

    fn open(path: &std::path::Path) -> File {
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .unwrap()
    }
    fn busy(path: &std::path::Path) {
        assert_eq!(
            FileExt::try_lock_exclusive(&open(path)).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }

    /// Hosted C2: close-only keeps a dead logical writer Busy; explicit original-owner unlock releases it without weakening active-writer exclusion.
    #[test]
    fn native_inherited_descriptor_lifetime() {
        if let Ok(mode) = std::env::var("OMNI_OWNED_FLOCK_HELPER") {
            helper(&mode);
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("writer.lock");
        // Positive historical control: actual close-only behavior remains locked until the unrelated helper closes its copy.
        let raw = open(&path);
        FileExt::try_lock_exclusive(&raw).unwrap();
        let inherited = Helper::start(&raw, "hold", std::process::id());
        drop(raw);
        busy(&path);
        inherited.finish();
        let probe = open(&path);
        FileExt::try_lock_exclusive(&probe).unwrap();
        FileExt::unlock(&probe).unwrap();
        drop(probe);
        // Actual production guard: active owner still excludes competing writers; only its own completion releases the OFD.
        let file = open(&path);
        FileExt::try_lock_exclusive(&file).unwrap();
        let guard = OwnedWriterLock::new(file);
        let mut inherited = Helper::start(guard.file.as_ref().unwrap(), "hold", std::process::id());
        busy(&path);
        drop(guard);
        let next = open(&path);
        FileExt::try_lock_exclusive(&next).unwrap();
        assert!(
            inherited.child.try_wait().unwrap().is_none(),
            "helper must still hold its real inherited descriptor at reacquisition"
        );
        FileExt::unlock(&next).unwrap();
        drop(next);
        inherited.finish();
        // PID control: dropping an inherited Rust guard after exec must not prematurely unlock the original live owner.
        let file = open(&path);
        FileExt::try_lock_exclusive(&file).unwrap();
        let guard = OwnedWriterLock::new(file);
        let inherited = Helper::start(
            guard.file.as_ref().unwrap(),
            "child-guard-drop",
            std::process::id(),
        );
        busy(&path);
        inherited.finish();
        busy(&path);
        drop(guard);
        let next = open(&path);
        FileExt::try_lock_exclusive(&next).unwrap();
        FileExt::unlock(&next).unwrap();
        // Transfer control: an unresolved native owner retains the actual file; lexical guard completion must not unlock it.
        let file = open(&path);
        FileExt::try_lock_exclusive(&file).unwrap();
        let guard = OwnedWriterLock::new(file);
        let retained = guard.into_file();
        busy(&path);
        FileExt::unlock(&retained).unwrap();
        drop(retained);
        let next = open(&path);
        FileExt::try_lock_exclusive(&next).unwrap();
        FileExt::unlock(&next).unwrap();
    }
}
