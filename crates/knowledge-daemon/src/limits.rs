//! Status-stream admission arithmetic.
//!
//! The HTTP task calls this before every delivery. A blocked write, the frame
//! cap, or the duration cap stops the stream. Stopping does not cancel the job.

use std::time::{Duration, Instant};

/// Owner limits for `POST /v1/status-stream`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StreamLimits {
    /// Active status streams for the whole process.
    pub max_connections: usize,
    /// Frames on one stream, including the first snapshot.
    pub max_frames: u32,
    /// Time from stream start, not extended by a slow client.
    pub max_duration: Duration,
    /// Minimum gap after the first frame.
    pub min_interval: Duration,
    /// How long one frame write may block before the stream stops.
    pub write_timeout: Duration,
}

impl StreamLimits {
    /// The service owner's status-stream numbers.
    pub const fn specified() -> Self {
        Self {
            max_connections: 16,
            max_frames: 32,
            max_duration: Duration::from_secs(30),
            min_interval: Duration::from_secs(1),
            write_timeout: Duration::from_secs(1),
        }
    }
}

/// What the stream may do at `now`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    /// Send one snapshot now.
    Deliver,
    /// Wait until `at` and check again.
    Wait {
        /// Earliest following delivery.
        at: Instant,
    },
    /// End the stream without a further domain effect.
    Stop,
}

/// Decide the next status frame.
pub fn next_step(
    limits: StreamLimits,
    started: Instant,
    frames_sent: u32,
    last_sent: Option<Instant>,
    now: Instant,
    write_blocked: bool,
) -> Step {
    if write_blocked
        || frames_sent >= limits.max_frames
        || now.saturating_duration_since(started) >= limits.max_duration
    {
        return Step::Stop;
    }
    if frames_sent == 0 {
        return Step::Deliver;
    }
    let last = last_sent.unwrap_or(started);
    let earliest = last + limits.min_interval;
    if now < earliest {
        Step::Wait { at: earliest }
    } else {
        Step::Deliver
    }
}

#[cfg(test)]
mod tests {
    use super::{Step, StreamLimits, next_step};
    use std::time::{Duration, Instant};

    #[test]
    fn specified_caps_stop_on_frames_duration_and_blocked_write() {
        let limits = StreamLimits::specified();
        assert_eq!(limits.max_connections, 16);
        assert_eq!(limits.max_frames, 32);
        assert_eq!(limits.max_duration, Duration::from_secs(30));
        assert_eq!(limits.min_interval, Duration::from_secs(1));
        assert_eq!(limits.write_timeout, Duration::from_secs(1));
        let started = Instant::now();
        assert_eq!(
            next_step(limits, started, 0, None, started, true),
            Step::Stop
        );
        assert_eq!(
            next_step(limits, started, 32, Some(started), started, false),
            Step::Stop
        );
        assert_eq!(
            next_step(
                limits,
                started,
                1,
                Some(started),
                started + Duration::from_secs(30),
                false
            ),
            Step::Stop
        );
        assert_eq!(
            next_step(limits, started, 0, None, started, false),
            Step::Deliver
        );
        assert_eq!(
            next_step(
                limits,
                started,
                1,
                Some(started),
                started + Duration::from_millis(200),
                false
            ),
            Step::Wait {
                at: started + Duration::from_secs(1)
            }
        );
    }
}
