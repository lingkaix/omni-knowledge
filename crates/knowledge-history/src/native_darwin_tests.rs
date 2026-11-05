//! Actual Darwin owned-child observations, not absent-BSD or Linux qualification.
use super::*;

fn child(program: &str, args: &[&str]) -> Child {
    let mut command = Command::new(program);
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    configure(&mut command, 1 << 20).unwrap();
    command.spawn().unwrap()
}

#[test]
fn owned_unreaped_exit_collects_without_a_bsd_start_sample() {
    let mut child = child("/usr/bin/true", &[]);
    let identity = ProcessIdentity {
        pid: child.id(),
        start: None,
    };
    let end = Instant::now() + Duration::from_secs(2);
    while !exited(identity.pid).unwrap() && Instant::now() < end {
        std::thread::sleep(Duration::from_millis(2));
    }
    let witnessed = exited(identity.pid).unwrap();
    let sample = group_sample(identity);
    let collected = collect(&mut child, identity, Duration::from_secs(2));
    // This controlled fixture executes only true; clean its owned handle even on a regression.
    let status = child.wait().unwrap();
    assert!(witnessed);
    assert_eq!(sample.unwrap().live, 0);
    collected.unwrap();
    assert!(status.success());
}

#[test]
fn missing_start_sample_does_not_authorize_a_live_group() {
    let mut child = child("/bin/sleep", &["20"]);
    let actual = process_identity(child.id()).unwrap();
    let unidentified = ProcessIdentity {
        pid: child.id(),
        start: None,
    };
    let refusal = checked_leader_exit(unidentified);
    let sampled = group_sample(unidentified);
    let collected = collect(&mut child, actual, Duration::from_secs(2));
    collected.unwrap();
    assert!(refusal.is_err());
    assert!(sampled.is_err());
}
