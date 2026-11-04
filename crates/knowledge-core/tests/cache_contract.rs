//! Public cache contract. Expectations come from storage's cache, identity, and
//! recovery clauses, the knowledge-core cache guide, and the rendered cache API.
//! The mapping premise is not provider discovery or service authorization.
//! No assertion treats garbage collection, eviction, or deletion as implemented.

#[allow(dead_code)]
mod support {
    include!("cache/support.rs");
}

use knowledge_core::cache::{
    Cache, CacheError, DirectoryIdentity, Locality, PinnedObject, ProcessorIdentity, Publication,
    SecurityDomain,
};
use knowledge_core::data::ContentDigest;
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use support::{
    CANARY, World, assert_input_free, assert_sha256_vectors, corrupt_regular_files_at_least,
    digest_of, file_is_symlink, generous_limits, original_unchanged, payload_sized_files,
    pin_object, processor, publish_bytes, read_exact, sha256_hex, snapshot, variant_name,
    write_private_file,
};

const DERIVED: &[u8] = b"derived-SENTINEL_cache_6c2e91-payload-v1";

#[test]
fn synthetic_digest_matches_published_sha256_vectors() {
    assert_sha256_vectors();
    assert_ne!(sha256_hex(b"abc"), "not-a-digest");
}

#[test]
fn processor_fingerprint_frames_order_options_and_dependencies() {
    assert_sha256_vectors();
    let transcript = b"corrected-transcript-v1";
    let annotation = b"embedding-annotation-v1";
    let forward = ordered_processor(&["zeta", "alpha"], "1.10", transcript, annotation);
    let reverse = ordered_processor(&["alpha", "zeta"], "1.10", transcript, annotation);
    let forward_id = forward.fingerprint(1024 * 1024).expect("fingerprint");
    let reverse_id = reverse.fingerprint(1024 * 1024).expect("fingerprint");
    assert_eq!(
        forward_id, reverse_id,
        "map insertion order is not identity"
    );

    assert_ne!(
        fingerprint(&processor("1.10", transcript, annotation)),
        fingerprint(&processor("1.1", transcript, annotation)),
        "numeric option spellings stay exact"
    );
    assert_ne!(
        fingerprint(&processor("1", transcript, annotation)),
        fingerprint(&processor("1.0", transcript, annotation))
    );
    assert_ne!(
        fingerprint(&processor("01", transcript, annotation)),
        fingerprint(&processor("1", transcript, annotation))
    );

    let other_transcript = processor("1.10", b"corrected-transcript-v2", annotation);
    let other_annotation = processor("1.10", transcript, b"embedding-annotation-v2");
    assert_ne!(fingerprint(&forward), fingerprint(&other_transcript));
    assert_ne!(fingerprint(&forward), fingerprint(&other_annotation));

    let mut swapped_roles = forward.clone();
    let transcript_digest = swapped_roles
        .dependencies
        .get("corrected-transcript")
        .expect("transcript")
        .clone();
    let annotation_digest = swapped_roles
        .dependencies
        .get("embedding-annotation")
        .expect("annotation")
        .clone();
    swapped_roles
        .dependencies
        .insert("corrected-transcript".to_string(), annotation_digest);
    swapped_roles
        .dependencies
        .insert("embedding-annotation".to_string(), transcript_digest);
    assert_ne!(fingerprint(&forward), fingerprint(&swapped_roles));

    let mut bare = forward.clone();
    bare.model = None;
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.package = None;
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.space = None;
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.dimensions = Some(9);
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.dimensions = None;
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.preprocessing
        .insert("order".to_string(), "segment,decode".to_string());
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.code = digest_of(b"other-processor-code");
    assert_ne!(fingerprint(&forward), fingerprint(&bare));
    bare = forward.clone();
    bare.version = "1.0".to_string();
    assert_ne!(fingerprint(&forward), fingerprint(&bare));

    let mut empty = forward.clone();
    empty.version.clear();
    let error = empty.fingerprint(1024 * 1024).expect_err("empty version");
    assert!(matches!(error, CacheError::Invalid));
    assert_input_free(&error);

    let mut huge = forward.clone();
    huge.options.insert("blob".to_string(), "x".repeat(10_000));
    let error = huge.fingerprint(64).expect_err("oversized identity");
    assert!(matches!(error, CacheError::Budget));
    assert_input_free(&error);
}

#[test]
fn same_bytes_do_not_admit_another_source_or_a_naked_digest() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    let installed = publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    assert_eq!(installed, Publication::Installed);
    original_unchanged(&world);

    let digest = world.premise.content();
    world.premise.clear_resolves();
    let (library, admitted, processor) = world.premise.binding();
    assert_eq!(admitted, support::SOURCE);
    let refused = closed_pin(cache.pin(&library, "source-2", &processor), "other source");
    assert_input_free(&refused);
    assert!(
        world
            .premise
            .resolves()
            .iter()
            .any(|(_, source)| source == "source-2"),
        "pin did not resolve source-2: {}",
        variant_name(&refused)
    );
    let bytes = read_exact(&cache, &world.premise, DERIVED.len()).expect("admitted read");
    assert_eq!(bytes, DERIVED);
    assert_ne!(bytes, world.original);

    world.premise.set_content(digest_of(b"other-asset-bytes"));
    assert_ne!(world.premise.content(), digest);
    let other_asset = publish_bytes(&cache, &world.premise, DERIVED).expect("other asset");
    assert_eq!(
        other_asset,
        Publication::Installed,
        "equal derived bytes were treated as the same asset"
    );
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("other asset read"),
        DERIVED
    );
    world.premise.set_content(digest);
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("first asset"),
        DERIVED
    );
    world.lab.finish();
}

#[test]
fn private_local_root_admits_outside_every_root_class() {
    let world = World::create(DERIVED);
    let before = snapshot(&world.premise.cache_dir());
    let cache = world.open(generous_limits()).expect("local root");
    let installed = publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    assert_eq!(installed, Publication::Installed);
    let read = read_exact(&cache, &world.premise, DERIVED.len()).expect("read");
    assert_eq!(read, DERIVED);
    assert!(snapshot(&world.premise.cache_dir()).len() > before.len());
    let usage = cache.usage().expect("usage");
    assert!(usage.charged_bytes > 0);
    assert!(usage.entries > 0);
    world.lab.finish();
}

#[test]
fn overlap_with_each_root_class_refuses_before_effects() {
    overlap_parent("home");
    overlap_parent("mount");
    overlap_parent("cloud");
    overlap_parent("history");
    prohibited_directory_inside_cache();
    lexical_alias_of_the_cache_refuses();
}

#[test]
fn symlink_permission_and_untrusted_locality_refuse_before_effects() {
    let world = World::create(DERIVED);
    let real = world.lab.root().join("real-cache");
    support::make_private_dir(&real);
    let link = world.lab.root().join("linked-cache");
    symlink(&real, &link).expect("symlink");
    let mut limits = generous_limits();
    limits.max_entries = 64;
    let premise = world.premise.clone();
    let before = snapshot(&real);
    let error = closed_open(Cache::open(&link, premise, limits), "symlink root");
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&real), before);

    let nested = world.lab.root().join("nested-real");
    support::make_private_dir(&nested);
    let linked_parent = world.lab.root().join("linked-parent");
    symlink(world.lab.root(), &linked_parent).expect("parent symlink");
    let through_link = linked_parent.join("nested-real");
    let premise = world.premise.clone();
    premise.set_cache_identity(DirectoryIdentity::observe(&nested).expect("nested identity"));
    let before = snapshot(&nested);
    let error = closed_open(
        Cache::open(&through_link, premise, limits),
        "symlink component",
    );
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&nested), before);

    for mode in [0o755, 0o707, 0o600] {
        let cache_dir = world.premise.cache_dir();
        let before = snapshot(&cache_dir);
        support::force_mode(&cache_dir, mode);
        let error = closed_open(world.open(limits), "mode");
        assert!(
            matches!(error, CacheError::UnsafeRoot),
            "{mode:o} {}",
            variant_name(&error)
        );
        assert_input_free(&error);
        assert_eq!(snapshot(&cache_dir), before);
        support::force_mode(&cache_dir, 0o700);
    }

    let cache_dir = world.premise.cache_dir();
    world.premise.set_cache_identity(
        DirectoryIdentity::observe(&cache_dir).expect("identity after mode restore"),
    );
    world.premise.set_locality(Locality::Untrusted);
    let before = snapshot(&cache_dir);
    let error = closed_open(world.open(limits), "untrusted");
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);
    world
        .premise
        .set_locality(Locality::ExplicitLocalAlternative);
    world.premise.set_cache_identity(
        DirectoryIdentity::observe(&cache_dir).expect("identity for the explicit alternative"),
    );
    let cache = world.open(limits).expect("explicit local alternative");
    assert_eq!(
        publish_bytes(&cache, &world.premise, DERIVED).expect("publish"),
        Publication::Installed
    );
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("alternative read"),
        DERIVED
    );
    world.lab.finish();
}

#[test]
fn unresolved_or_symlink_prohibited_roots_fail_closed_without_effects() {
    // Public slice text: unavailable or symlink-spelled prohibited roots fail
    // closed. This is not a storage amendment that offline roots are outside
    // the cache, and it is not a root detector.
    let world = World::create(DERIVED);
    let cache_dir = world.premise.cache_dir();
    let dangling = world.lab.root().join("offline-root");
    symlink(world.lab.root().join("missing-target"), &dangling).expect("dangling");
    let mut prohibited = world.premise.prohibited();
    prohibited.push(dangling);
    world.premise.set_prohibited(prohibited);
    let before = snapshot(&cache_dir);
    let error = closed_open(world.open(generous_limits()), "symlink prohibited");
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);

    let absent = world.lab.root().join("absent-prohibited");
    let mut prohibited = world.premise.prohibited();
    prohibited.push(absent);
    world.premise.set_prohibited(prohibited);
    let error = closed_open(world.open(generous_limits()), "absent prohibited");
    assert!(
        matches!(error, CacheError::UnsafeRoot | CacheError::Missing),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);
    world.lab.finish();
}

#[test]
fn publish_is_atomic_no_clobber_and_range_verified() {
    let world = World::create(DERIVED);
    let mut limits = generous_limits();
    limits.max_read_bytes = 8;
    let cache = world.open(limits).expect("open");
    assert_eq!(
        publish_bytes(&cache, &world.premise, DERIVED).expect("publish"),
        Publication::Installed
    );
    let again = publish_bytes(&cache, &world.premise, DERIVED).expect("duplicate");
    assert_eq!(again, Publication::AlreadyPresent);
    let pin = pin_object(&cache, &world.premise).expect("pin");
    assert_eq!(pin.read(0, 8).expect("head"), &DERIVED[..8]);
    assert_eq!(pin.read(8, 8).expect("middle"), &DERIVED[8..16]);
    let tail_at = DERIVED.len() as u64 - 4;
    assert_eq!(
        pin.read(tail_at, 8).expect("short tail"),
        &DERIVED[DERIVED.len() - 4..]
    );
    assert_eq!(pin.read(DERIVED.len() as u64, 4).expect("eof"), b"");
    let over = pin.read(0, 9).expect_err("read ceiling");
    assert!(
        matches!(over, CacheError::Budget),
        "{}",
        variant_name(&over)
    );
    assert_input_free(&over);

    let conflict = b"derived-SENTINEL_cache_6c2e91-other";
    let error = publish_bytes(&cache, &world.premise, conflict).expect_err("conflict");
    assert!(
        matches!(error, CacheError::Conflict),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    let pin = pin_object(&cache, &world.premise).expect("still pinned");
    let mut whole = Vec::new();
    let mut offset = 0u64;
    while whole.len() < DERIVED.len() {
        let chunk = pin.read(offset, 8).expect("chunk");
        assert!(!chunk.is_empty() || offset as usize == DERIVED.len());
        if chunk.is_empty() {
            break;
        }
        offset += chunk.len() as u64;
        whole.extend(chunk);
    }
    assert_eq!(whole, DERIVED);
    world.lab.finish();
}

#[test]
fn corrupt_objects_are_not_overwritten_or_repaired() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    let cache_dir = world.premise.cache_dir();
    let changed = corrupt_regular_files_at_least(&cache_dir, DERIVED.len());
    assert!(
        changed > 0,
        "observer found no payload-sized cache file to corrupt"
    );
    let mutated = snapshot(&cache_dir);
    match pin_object(&cache, &world.premise) {
        Err(error) => assert_input_free(&error),
        Ok(pin) => {
            let error = pin.read(0, 1).expect_err("corrupt range");
            assert_input_free(&error);
        }
    }
    let republish = publish_bytes(&cache, &world.premise, DERIVED);
    assert!(
        republish.is_err(),
        "corrupt complete object was replaced: {republish:?}"
    );
    if let Err(error) = &republish {
        assert_input_free(error);
    }
    let after = snapshot(&cache_dir);
    for (path, node) in &mutated {
        let support::Node::File { bytes } = node else {
            continue;
        };
        if bytes.len() < DERIVED.len() {
            continue;
        }
        match after.get(path) {
            Some(support::Node::File { bytes: now }) => {
                assert_eq!(
                    now,
                    bytes,
                    "corrupt object at {} was repaired",
                    path.display()
                );
            }
            other => panic!(
                "corrupt object at {} was removed: {other:?}",
                path.display()
            ),
        }
    }
    match pin_object(&cache, &world.premise) {
        Err(error) => assert_input_free(&error),
        Ok(pin) => {
            let error = pin
                .read(0, DERIVED.len())
                .expect_err("corrupt object became readable");
            assert_input_free(&error);
        }
    }
    world.lab.finish();
}

#[test]
fn cancelled_failing_and_over_budget_producers_keep_the_prior_object() {
    let world = World::create(DERIVED);
    let mut limits = generous_limits();
    limits.max_object_bytes = 64;
    limits.max_disk_bytes = 32 * 1024;
    let cache = world.open(limits).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("prior");
    let before = snapshot(&world.premise.cache_dir());
    let usage_before = cache.usage().expect("usage");

    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let mut producer = ChunkThen {
        data: vec![b'x'; 48],
        pos: 0,
        chunk: 8,
        arm: flag,
        fail: false,
        canary: CANARY,
    };
    let (library, source, processor) = world.premise.binding();
    let cancelled = cache.publish(&library, &source, &processor, &mut producer, || {
        cancel.load(Ordering::SeqCst)
    });
    let error = cancelled.expect_err("cancelled");
    assert!(
        matches!(error, CacheError::Cancelled),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );

    let mut failing = ChunkThen {
        data: vec![b'y'; 48],
        pos: 0,
        chunk: 8,
        arm: Arc::new(AtomicBool::new(false)),
        fail: true,
        canary: CANARY,
    };
    let error = cache
        .publish(&library, &source, &processor, &mut failing, || false)
        .expect_err("producer failure");
    assert!(matches!(error, CacheError::Io), "{}", variant_name(&error));
    assert_input_free(&error);
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );

    let huge = vec![b'z'; 80];
    let error = publish_bytes(&cache, &world.premise, &huge).expect_err("object budget");
    assert!(
        matches!(error, CacheError::Budget),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );

    let usage = cache.usage().expect("usage");
    assert!(usage.retained_candidates > usage_before.retained_candidates);
    assert!(snapshot(&world.premise.cache_dir()).len() > before.len());
    assert!(payload_sized_files(&world.premise.cache_dir(), DERIVED.len()) >= 1);
    world.lab.finish();
}

#[test]
fn mapping_recipe_and_root_changes_refuse_bytes_without_dropping_the_object() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    let pin = pin_object(&cache, &world.premise).expect("pin");

    world.premise.set_revision(2);
    let error = pin.read(0, DERIVED.len()).expect_err("revision");
    assert_input_free(&error);
    world.premise.set_revision(1);
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    world.premise.set_content(digest_of(b"different-original"));
    let error = pin.read(0, 4).expect_err("content");
    assert_input_free(&error);
    world.premise.set_content(digest_of(&world.original));
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    world.premise.set_domain(SecurityDomain::Exposed);
    let error = pin.read(0, 4).expect_err("domain");
    assert_input_free(&error);
    world.premise.set_domain(SecurityDomain::Sensitive);
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    world.premise.set_refuse(true);
    let error = pin.read(0, 4).expect_err("policy");
    assert_input_free(&error);
    world.premise.set_refuse(false);
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    let mount = world.lab.root().join("mount");
    let foreign_home = DirectoryIdentity::observe(&mount).expect("mount identity");
    world.premise.set_home_identity(foreign_home);
    let error = pin.read(0, 4).expect_err("copied home");
    assert_input_free(&error);
    let home = DirectoryIdentity::observe(&world.premise.home()).expect("home identity");
    world.premise.set_home_identity(home);
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    let changed = processor("1.10", b"other-transcript", b"embedding-annotation-v1");
    world.premise.set_processor(changed.clone());
    let error = pin.read(0, 4).expect_err("recipe");
    assert_input_free(&error);
    let missing = cache.pin(support::LIBRARY, support::SOURCE, &changed);
    assert!(missing.is_err(), "new recipe returned the old object");
    world.premise.set_processor(processor(
        "1.10",
        b"corrected-transcript-v1",
        b"embedding-annotation-v1",
    ));
    assert_eq!(pin.read(0, DERIVED.len()).expect("restored"), DERIVED);

    let mut prohibited = world.premise.prohibited();
    prohibited.push(world.premise.cache_dir());
    world.premise.set_prohibited(prohibited);
    let error = pin.read(0, 4).expect_err("root set");
    assert!(
        matches!(error, CacheError::UnsafeRoot | CacheError::Changed),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    world.lab.finish();
}

#[test]
fn captured_root_replacement_does_not_redirect_effects() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    let current = world.premise.cache_dir();
    let retired = world.lab.root().join("retired-cache");
    fs::rename(&current, &retired).expect("rename root");
    support::make_private_dir(&current);
    let fresh = snapshot(&current);
    let error = publish_bytes(&cache, &world.premise, b"redirected").expect_err("replaced root");
    assert!(
        matches!(
            error,
            CacheError::Changed | CacheError::UnsafeRoot | CacheError::Io
        ),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(
        snapshot(&current),
        fresh,
        "effects moved to the replacement root"
    );
    assert!(payload_sized_files(&retired, DERIVED.len()) >= 1);
    let read = pin_object(&cache, &world.premise);
    assert!(read.is_err(), "replacement root supplied the old bytes");
    world.lab.finish();
}

#[test]
fn namespaces_isolate_domain_library_and_copied_home() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    let sensitive = DERIVED;
    let unexposed = b"unexposed-SENTINEL_cache_6c2e91-v1";
    let exposed = b"exposed-SENTINEL_cache_6c2e91-v1";
    publish_bytes(&cache, &world.premise, sensitive).expect("sensitive");
    world.premise.set_domain(SecurityDomain::Unexposed);
    assert_eq!(
        publish_bytes(&cache, &world.premise, unexposed).expect("unexposed"),
        Publication::Installed
    );
    world.premise.set_domain(SecurityDomain::Exposed);
    assert_eq!(
        publish_bytes(&cache, &world.premise, exposed).expect("exposed"),
        Publication::Installed
    );
    assert_eq!(
        read_exact(&cache, &world.premise, exposed.len()).expect("exposed read"),
        exposed
    );
    world.premise.set_domain(SecurityDomain::Unexposed);
    assert_eq!(
        read_exact(&cache, &world.premise, unexposed.len()).expect("unexposed read"),
        unexposed
    );
    world.premise.set_domain(SecurityDomain::Sensitive);
    assert_eq!(
        read_exact(&cache, &world.premise, sensitive.len()).expect("sensitive read"),
        sensitive
    );

    world.premise.set_library("library-2");
    let other = b"library-2-SENTINEL_cache_6c2e91";
    assert_eq!(
        publish_bytes(&cache, &world.premise, other).expect("other library"),
        Publication::Installed
    );
    assert_eq!(
        read_exact(&cache, &world.premise, other.len()).expect("lib2"),
        other
    );
    world.premise.set_library(support::LIBRARY);
    assert_eq!(
        read_exact(&cache, &world.premise, sensitive.len()).expect("lib1"),
        sensitive
    );

    let copied = world.lab.root().join("copied-home");
    support::make_private_dir(&copied);
    write_private_file(&copied.join("original.bin"), &world.original);
    let copied_identity = DirectoryIdentity::observe(&copied).expect("copied identity");
    let copied_home = std::path::absolute(&copied).expect("copied home");
    world.premise.set_home(copied_home.clone());
    world.premise.set_home_identity(copied_identity);
    let mut prohibited = world.premise.prohibited();
    prohibited.push(copied_home);
    world.premise.set_prohibited(prohibited);
    let copied_bytes = b"copied-home-SENTINEL_cache_6c2e91";
    assert_eq!(
        publish_bytes(&cache, &world.premise, copied_bytes).expect("copied library"),
        Publication::Installed
    );
    assert_eq!(
        read_exact(&cache, &world.premise, copied_bytes.len()).expect("copied read"),
        copied_bytes
    );
    let original_home_path = world.lab.root().join("home");
    let original_home = DirectoryIdentity::observe(&original_home_path).expect("home");
    world
        .premise
        .set_home(std::path::absolute(&original_home_path).expect("home path"));
    world.premise.set_home_identity(original_home);
    assert_eq!(
        read_exact(&cache, &world.premise, sensitive.len()).expect("original home"),
        sensitive
    );
    world.lab.finish();
}

#[test]
fn unknown_entries_symlinks_and_failed_candidates_are_retained() {
    let world = World::create(DERIVED);
    let cache = world.open(generous_limits()).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    let cache_dir = world.premise.cache_dir();
    let unknown = cache_dir.join("unknown-note.bin");
    let marker = b"keep-unknown-SENTINEL_cache_6c2e91";
    write_private_file(&unknown, marker);
    let outside = world.lab.root().join("outside-target");
    let outside_bytes = b"outside-target-bytes";
    write_private_file(&outside, outside_bytes);
    let link = cache_dir.join("unknown-link");
    symlink(&outside, &link).expect("symlink");
    let usage = cache.usage().expect("usage");
    assert!(usage.unknown_entries >= 2);
    assert_eq!(fs::read(&unknown).expect("unknown"), marker);
    assert!(file_is_symlink(&link));
    assert_eq!(fs::read(&outside).expect("target"), outside_bytes);
    assert_eq!(fs::read_link(&link).expect("link"), outside);

    drop(cache);
    assert_eq!(fs::read(&unknown).expect("unknown after drop"), marker);
    assert!(file_is_symlink(&link));
    let cache = world.open(generous_limits()).expect("reopen");
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("reopen read"),
        DERIVED
    );

    let installed = world.premise.processor();
    world.premise.set_processor(processor(
        "7.2",
        b"corrected-transcript-v1",
        b"embedding-annotation-v1",
    ));
    let cancel = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&cancel);
    let mut producer = ChunkThen {
        data: vec![b'q'; 24],
        pos: 0,
        chunk: 8,
        arm: flag,
        fail: false,
        canary: CANARY,
    };
    let (library, source, processor) = world.premise.binding();
    let error = cache
        .publish(&library, &source, &processor, &mut producer, || {
            cancel.load(Ordering::SeqCst)
        })
        .expect_err("cancel");
    assert!(
        matches!(error, CacheError::Cancelled),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    world.premise.set_processor(installed);
    let retained = cache.usage().expect("usage").retained_candidates;
    assert!(retained >= 1);
    let files = snapshot(&cache_dir);
    drop(cache);
    let cache = world
        .open(generous_limits())
        .expect("reopen after candidate");
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );
    assert!(cache.usage().expect("usage").retained_candidates >= 1);
    assert_eq!(snapshot(&cache_dir), files);
    assert_eq!(fs::read(&unknown).expect("still unknown"), marker);
    assert!(file_is_symlink(&link));
    assert_eq!(fs::read(&outside).expect("target"), outside_bytes);

    let nested = cache_dir.join("unknown-dir");
    support::make_private_dir(&nested);
    let nested_file = nested.join("nested.bin");
    write_private_file(&nested_file, b"nested-keep");
    let error = cache.usage().expect_err("unknown directory");
    assert!(
        matches!(error, CacheError::InspectionRequired),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(fs::read(&nested_file).expect("nested"), b"nested-keep");
    drop(cache);
    match world.open(generous_limits()) {
        Ok(opened) => {
            assert_eq!(
                read_exact(&opened, &world.premise, DERIVED.len())
                    .expect("read beside unknown dir"),
                DERIVED
            );
            let usage = opened.usage().expect_err("directory still present");
            assert!(
                matches!(usage, CacheError::InspectionRequired),
                "{}",
                variant_name(&usage)
            );
            assert_input_free(&usage);
        }
        Err(error) => {
            assert!(
                matches!(error, CacheError::InspectionRequired),
                "{}",
                variant_name(&error)
            );
            assert_input_free(&error);
        }
    }
    assert_eq!(
        fs::read(&nested_file).expect("cache deleted the unknown directory"),
        b"nested-keep"
    );
    assert_eq!(fs::read(&unknown).expect("unknown"), marker);
    fs::remove_dir_all(&nested).expect("caller removes only its unknown directory");
    let reopened = world
        .open(generous_limits())
        .expect("reopen after the caller removes the unknown directory");
    assert_eq!(
        read_exact(&reopened, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );
    assert_eq!(fs::read(&unknown).expect("still unknown"), marker);
    assert!(file_is_symlink(&link));
    assert_eq!(fs::read(&outside).expect("target"), outside_bytes);
    world.lab.finish();
}

#[test]
fn finite_entry_disk_pin_and_io_limits_refuse() {
    let world = World::create(DERIVED);
    let cache_dir = world.premise.cache_dir();
    for index in 0..12 {
        write_private_file(&cache_dir.join(format!("extra-{index}.bin")), b"u");
    }
    let before = snapshot(&cache_dir);
    let mut tight = generous_limits();
    tight.max_entries = 3;
    let error = closed_open(world.open(tight), "entry ceiling");
    assert!(
        matches!(error, CacheError::Budget | CacheError::Invalid),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);
    for index in 0..12 {
        fs::remove_file(cache_dir.join(format!("extra-{index}.bin"))).expect("remove extra");
    }

    let mut zero_buffer = generous_limits();
    zero_buffer.buffer_bytes = 0;
    let before = snapshot(&cache_dir);
    let error = closed_open(world.open(zero_buffer), "zero buffer");
    assert!(
        matches!(error, CacheError::Invalid | CacheError::Budget),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);

    let mut pins = generous_limits();
    pins.max_pins = 1;
    let cache = world.open(pins).expect("open");
    publish_bytes(&cache, &world.premise, DERIVED).expect("publish");
    let first = pin_object(&cache, &world.premise).expect("first pin");
    let error = closed_pin(pin_object(&cache, &world.premise), "pin ceiling");
    assert!(
        matches!(error, CacheError::Budget),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    drop(first);
    let second = pin_object(&cache, &world.premise).expect("pin after release");
    assert_eq!(second.read(0, DERIVED.len()).expect("read"), DERIVED);
    drop(second);
    drop(cache);

    let mut identity = generous_limits();
    identity.max_identity_bytes = 8;
    let before = snapshot(&cache_dir);
    match world.open(identity) {
        Err(error) => {
            assert_input_free(&error);
            assert_eq!(snapshot(&cache_dir), before);
        }
        Ok(cache) => {
            let error =
                publish_bytes(&cache, &world.premise, DERIVED).expect_err("identity ceiling");
            assert!(
                matches!(error, CacheError::Budget | CacheError::Invalid),
                "{}",
                variant_name(&error)
            );
            assert_input_free(&error);
            assert!(pin_object(&cache, &world.premise).is_err());
        }
    }

    let mut disk_limits = generous_limits();
    disk_limits.max_disk_bytes = 32 * 1024;
    disk_limits.max_object_bytes = 1024 * 1024;
    let cache = world.open(disk_limits).expect("disk open");
    let installed = world.premise.processor();
    world.premise.set_processor(processor(
        "8.1",
        b"corrected-transcript-v1",
        b"embedding-annotation-v1",
    ));
    let error =
        publish_bytes(&cache, &world.premise, &vec![b'd'; 40 * 1024]).expect_err("disk budget");
    assert!(
        matches!(error, CacheError::Budget),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    world.premise.set_processor(installed);
    assert_eq!(
        read_exact(&cache, &world.premise, DERIVED.len()).expect("prior"),
        DERIVED
    );
    drop(cache);

    world.premise.set_processor(processor(
        "9.9",
        b"corrected-transcript-v1",
        b"embedding-annotation-v1",
    ));
    let cache = Arc::new(world.open(generous_limits()).expect("open"));
    let started = Arc::new(AtomicBool::new(false));
    let release = Arc::new(AtomicBool::new(false));
    let started_flag = Arc::clone(&started);
    let release_flag = Arc::clone(&release);
    let premise = world.premise.clone();
    let worker_cache = Arc::clone(&cache);
    let blocked_payload = b"blocked-SENTINEL_cache_6c2e91".to_vec();
    let worker = thread::spawn(move || {
        let mut blocked = BlockThen {
            started: started_flag,
            release: release_flag,
            payload: blocked_payload,
            sent: false,
        };
        publish_with_reader(&worker_cache, &premise, &mut blocked)
    });
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while !started.load(Ordering::SeqCst) {
        assert!(
            std::time::Instant::now() < deadline,
            "observer timed out before the producer blocked"
        );
        thread::yield_now();
    }
    let lane = cache.usage();
    match lane {
        Err(error) => {
            assert!(
                matches!(error, CacheError::Busy),
                "{}",
                variant_name(&error)
            );
            assert_input_free(&error);
        }
        Ok(_) => panic!("reentrant usage ran during publish"),
    }
    let excluded = world.open(generous_limits());
    match excluded {
        Err(error) => {
            assert!(
                matches!(error, CacheError::Busy),
                "{}",
                variant_name(&error)
            );
            assert_input_free(&error);
        }
        Ok(_) => panic!("second owner was admitted during an active publish"),
    }
    release.store(true, Ordering::SeqCst);
    let published = worker.join().expect("worker");
    assert_eq!(published.expect("publish"), Publication::Installed);
    assert_eq!(
        read_exact(
            &cache,
            &world.premise,
            b"blocked-SENTINEL_cache_6c2e91".len()
        )
        .expect("blocked"),
        b"blocked-SENTINEL_cache_6c2e91"
    );
    world.lab.finish();
}

fn closed_open(result: Result<Cache<support::Premise>, CacheError>, what: &str) -> CacheError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{what} opened a cache"),
    }
}

fn closed_pin(
    result: Result<PinnedObject<support::Premise>, CacheError>,
    what: &str,
) -> CacheError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("{what} pinned an object"),
    }
}

fn fingerprint(processor: &ProcessorIdentity) -> ContentDigest {
    processor.fingerprint(1024 * 1024).expect("fingerprint")
}

fn ordered_processor(
    keys: &[&str],
    option: &str,
    transcript: &[u8],
    annotation: &[u8],
) -> ProcessorIdentity {
    let mut identity = processor(option, transcript, annotation);
    identity.options.clear();
    for key in keys {
        identity
            .options
            .insert((*key).to_string(), option.to_string());
    }
    identity
}

fn overlap_parent(class_name: &str) {
    let world = World::create(DERIVED);
    let parent = world.lab.root().join(class_name);
    let cache_dir = parent.join("nested-cache");
    support::make_private_dir(&cache_dir);
    let premise = relocated(&world, &cache_dir);
    let before = snapshot(&cache_dir);
    let error = closed_open(
        Cache::open(&cache_dir, premise, generous_limits()),
        class_name,
    );
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{class_name} {}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(
        snapshot(&cache_dir),
        before,
        "{class_name} wrote before refusal"
    );
    world.lab.finish();
}

fn prohibited_directory_inside_cache() {
    let world = World::create(DERIVED);
    let cache_dir = world.premise.cache_dir();
    let history = cache_dir.join("history-store");
    support::make_private_dir(&history);
    write_private_file(&history.join("events.bin"), b"history-bytes");
    let mut prohibited = world.premise.prohibited();
    prohibited.push(support_absolute(&history));
    world.premise.set_prohibited(prohibited);
    let before = snapshot(&cache_dir);
    let error = closed_open(world.open(generous_limits()), "history inside cache");
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);
    assert_eq!(
        fs::read(history.join("events.bin")).expect("history"),
        b"history-bytes"
    );
    world.lab.finish();
}

fn lexical_alias_of_the_cache_refuses() {
    let world = World::create(DERIVED);
    let cache_dir = world.premise.cache_dir();
    let sibling = world.lab.root().join("sibling");
    support::make_private_dir(&sibling);
    let alias = sibling.join("..").join("cache");
    let mut prohibited = world.premise.prohibited();
    prohibited.push(alias);
    world.premise.set_prohibited(prohibited);
    let before = snapshot(&cache_dir);
    let error = closed_open(world.open(generous_limits()), "lexical alias");
    assert!(
        matches!(error, CacheError::UnsafeRoot),
        "{}",
        variant_name(&error)
    );
    assert_input_free(&error);
    assert_eq!(snapshot(&cache_dir), before);
    world.lab.finish();
}

fn relocated(world: &World, cache_dir: &Path) -> support::Premise {
    let premise = world.premise.clone();
    premise.set_cache_identity(DirectoryIdentity::observe(cache_dir).expect("identity"));
    premise
}

fn support_absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).expect("absolute")
}

struct ChunkThen {
    data: Vec<u8>,
    pos: usize,
    chunk: usize,
    arm: Arc<AtomicBool>,
    fail: bool,
    canary: &'static str,
}

impl Read for ChunkThen {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.fail && self.pos >= self.chunk {
            return Err(io::Error::other(self.canary));
        }
        if self.pos >= self.data.len() {
            return Ok(0);
        }
        let size = self.chunk.min(buf.len()).min(self.data.len() - self.pos);
        buf[..size].copy_from_slice(&self.data[self.pos..self.pos + size]);
        self.pos += size;
        if self.pos >= self.chunk {
            self.arm.store(true, Ordering::SeqCst);
        }
        Ok(size)
    }
}

struct BlockThen {
    started: Arc<AtomicBool>,
    release: Arc<AtomicBool>,
    payload: Vec<u8>,
    sent: bool,
}

impl Read for BlockThen {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if !self.sent {
            self.started.store(true, Ordering::SeqCst);
            while !self.release.load(Ordering::SeqCst) {
                thread::yield_now();
            }
            let size = self.payload.len().min(buf.len());
            buf[..size].copy_from_slice(&self.payload[..size]);
            self.sent = true;
            return Ok(size);
        }
        Ok(0)
    }
}

fn publish_with_reader(
    cache: &Cache<support::Premise>,
    premise: &support::Premise,
    reader: &mut impl Read,
) -> Result<Publication, CacheError> {
    let (library, source, processor) = premise.binding();
    cache.publish(&library, &source, &processor, reader, || false)
}
