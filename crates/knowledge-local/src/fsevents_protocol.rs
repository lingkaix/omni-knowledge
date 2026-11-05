//! Fixed-size helper transport and allocation-free native hint reduction, not source authority.
/// Exact atomic pipe frame size (less than POSIX PIPE_BUF).
pub const FRAME_BYTES: usize = 56;
/// A stream has actually started on its selected device and held root.
pub const READY: u32 = 1 << 31;
/// Coalescing, drops, wraps, mount and root changes cannot establish continuity.
pub const GAP: u32 = 0x0000_00ef; // HistoryDone (0x10) is not continuity loss.
/// Published native flags through ItemCloned; unknown flags cannot be filtered as irrelevant paths.
pub const KNOWN_FLAGS: u32 = 0x007f_ffff;
/// Convert an actual no-firmlink descriptor path to Apple's slash-free device-relative spelling.
/// A mount-prefix alias is refused; these bytes are setup metadata, never source authority.
pub fn relative_root<'a>(absolute: &'a [u8], mount: &[u8]) -> Option<&'a [u8]> {
    if mount == b"/" {
        return absolute.strip_prefix(b"/");
    }
    let suffix = absolute.strip_prefix(mount)?;
    if suffix.is_empty() {
        Some(suffix)
    } else {
        suffix.strip_prefix(b"/")
    }
}
/// Metadata only; paths and source bodies never cross this transport.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// Native selected device, not a portable volume identity.
    pub device: u64,
    /// Actual held root inode.
    pub root: u64,
    /// Native device event-store UUID.
    pub volume: [u8; 16],
    /// Observed device-relative native event ID; zero is never a replay point.
    pub event_id: u64,
    /// Native flags plus the explicit startup bit.
    pub flags: u32,
}
impl Frame {
    /// Encode one complete fixed-width little-endian frame without heap allocation.
    pub fn encode(self) -> [u8; FRAME_BYTES] {
        let mut b = [0; FRAME_BYTES];
        b[..8].copy_from_slice(b"OMNIFSE1");
        b[8..16].copy_from_slice(&self.device.to_le_bytes());
        b[16..24].copy_from_slice(&self.root.to_le_bytes());
        b[24..40].copy_from_slice(&self.volume);
        b[40..48].copy_from_slice(&self.event_id.to_le_bytes());
        b[48..52].copy_from_slice(&self.flags.to_le_bytes());
        b
    }
    /// Reject unknown framing/reserved bytes rather than claiming an empty healthy poll.
    pub fn decode(b: &[u8; FRAME_BYTES]) -> Option<Self> {
        if &b[..8] != b"OMNIFSE1" || b[52..] != [0; 4] {
            return None;
        }
        Some(Self {
            device: u64::from_le_bytes(b[8..16].try_into().ok()?),
            root: u64::from_le_bytes(b[16..24].try_into().ok()?),
            volume: b[24..40].try_into().ok()?,
            event_id: u64::from_le_bytes(b[40..48].try_into().ok()?),
            flags: u32::from_le_bytes(b[48..52].try_into().ok()?),
        })
    }
    /// Reduce one native event; regressing/zero IDs are gaps, never cursor advancement.
    pub fn event(&mut self, flags: u32, id: u64) {
        self.flags |= flags;
        if id == 0 || id == u64::MAX || id <= self.event_id || flags & !KNOWN_FLAGS != 0 {
            self.flags |= 1;
        }
        self.event_id = self.event_id.max(id);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    /// Portable byte/reduction proof only; this test does not execute any Apple framework.
    #[test]
    fn fixed_transport_retains_native_gaps_and_rejects_bad_frames() {
        assert_eq!(relative_root(b"/Users/a", b"/"), Some(&b"Users/a"[..]));
        assert_eq!(
            relative_root(b"/System/Volumes/Data/private/tmp", b"/System/Volumes/Data"),
            Some(&b"private/tmp"[..])
        );
        assert_eq!(
            relative_root(b"/Volumes/Data", b"/Volumes/Data"),
            Some(&b""[..])
        );
        assert!(relative_root(b"/Volumes/DataAlias/x", b"/Volumes/Data").is_none());
        let mut f = Frame {
            device: 17,
            root: 23,
            volume: [3; 16],
            event_id: 9,
            flags: 0,
        };
        assert_eq!(Frame::decode(&f.encode()), Some(f));
        for flag in [1, 2, 4, 8, 32, 64, 128] {
            let mut event = f;
            event.event(flag, 10);
            assert_ne!(event.flags & GAP, 0);
        }
        assert_eq!(0x10 & GAP, 0, "HistoryDone alone is not a dropped event");
        let mut unknown = f;
        unknown.event(0x0080_0000, 10);
        assert_ne!(unknown.flags & GAP, 0);
        f.event(0, 8);
        assert_eq!(f.event_id, 9);
        assert_ne!(f.flags & GAP, 0);
        let mut bad = f.encode();
        bad[52] = 1;
        assert!(Frame::decode(&bad).is_none());
        bad = f.encode();
        bad[0] = 0;
        assert!(Frame::decode(&bad).is_none());
    }
}
