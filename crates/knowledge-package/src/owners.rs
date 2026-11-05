//! Real storage, inert history, and ordered held EPUB calls.
//!
//! The package borrows the caller's store, history, policy, and claim.
//! It does not start a second Git engine, invent an approval record, or finish the caller's holding.
//! `knowledge_history::Error::Unsettled` stays typed until the caller collects it.
//! A display string is not collection and is not settlement.

use crate::{
    ApplyOutcome, ContentDigest, Error, GitAdmit, GitTransactions, OwnerHandoff, RelativePath,
    SafeApply, VerifiedSource, Work, admission_error, operation_id,
};
use knowledge_core::jobs::Claim;
use knowledge_history::{BundleError, BundleReport, Change, Destination, History, Request};
use knowledge_processing::funding::Funding;
use knowledge_processing::held::{EpubMember, HeldEpub, HeldSource, PartialOutputs};
use knowledge_processing::{AdmitError, Admitted};
use knowledge_storage::{
    Expected, ImportResult, Inspection, Phase, Preflight, Scope, Store, WritePolicy,
};
use sha2::Digest;
use std::fs::File;
use std::os::unix::fs::FileExt;
use std::path::Path;

/// Operation ids shared with history: at most 96 characters, starting with a letter or digit.
pub fn native_operation(value: &str) -> Result<(), Error> {
    operation_id(value)?;
    if value.len() > 96 || !value.as_bytes()[0].is_ascii_alphanumeric() {
        return Err(Error::Invalid);
    }
    Ok(())
}

/// One already handed read-only member. Slice order is the EPUB spine order.
pub struct HeldMember<'a> {
    /// Copied inode opened read-only. A path is not a substitute.
    pub file: &'a File,
    /// Digest of that descriptor, computed before this handoff.
    pub version: &'a ContentDigest,
    /// Publication name used only for link lookup.
    pub logical_name: &'a RelativePath,
}

/// Processing failure with its descriptors, holdings, and unknown child still attached.
#[derive(Debug)]
pub enum EpubOwnerError {
    /// Stopped artifacts, source descriptors, and live holdings.
    Partial(PartialOutputs),
    /// Unknown child collection. Call `collect` on this value; dropping it is not collection.
    Unsettled(knowledge_processing::worker::UnsettledChild),
    /// Credit or account refusal before a generated book.
    Admission(AdmitError),
    /// Another settled processing error. This is the original error, not its display text.
    Settled(anyhow::Error),
}

/// Copy the verified stream and import that read-only descriptor.
/// An existing destination returns [`knowledge_storage::Error::Conflict`] and is left in place.
/// The source file is not deleted. The call does not publish Git.
pub fn import_verified_source<P: WritePolicy>(
    source: &mut VerifiedSource,
    store: &Store<P>,
    operation: &str,
    destination: &str,
    private: &Path,
    preflight: &Preflight<'_>,
    work: &mut Work<'_>,
) -> Result<OwnerHandoff<Result<ImportResult, knowledge_storage::Error>>, Error> {
    native_operation(operation)?;
    if destination.is_empty() {
        return Err(Error::Invalid);
    }
    source.with_private_file(private, operation, work, |file, _digest, _work| {
        store.import_copy(operation, file, destination, preflight)
    })
}

/// Facts for one text installation. `expected` of `None` asks for an exclusive create.
pub struct TextInstall<'a> {
    /// Fresh operation id, also the private child directory name.
    pub operation: &'a str,
    /// Home-relative text path.
    pub path: &'a str,
    /// Current storage witness, or `None` when the path must be absent.
    pub expected: Option<knowledge_storage::Version>,
    /// Expected accepted history base, or `None` for an empty history.
    pub base: Option<knowledge_history::Oid>,
    /// Absolute private parent for the copied inode.
    pub private: &'a Path,
}

/// Copy the verified stream and install its UTF-8 text when `expected` still matches.
/// The cached source length is admitted with the text, digest encodings, request copies, and owner preparation before any operation or path copy and before the private handoff.
/// `read_text` uses that admission and does not charge it again. No descriptor is statted before this admission.
/// This calls `prepare` and, when that preparation is new, `install`.
/// It does not call `publish`. A stale witness returns conflict and leaves the destination bytes alone.
/// The admitted cost stays on the caller's holding through the private copy, preparation, install, and a typed refusal.
pub fn install_verified_text<P: WritePolicy>(
    source: &mut VerifiedSource,
    store: &Store<P>,
    history: &History,
    install: &TextInstall<'_>,
    scope: &Scope<'_>,
    work: &mut Work<'_>,
) -> Result<OwnerHandoff<Result<Inspection, knowledge_storage::Error>>, Error> {
    native_operation(install.operation)?;
    let length = source.len();
    if length > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    let peak = preparation_peak(length, install)?;
    crate::charge_fresh(work, peak, 0)?;
    if RelativePath::new(install.path.to_owned()).is_err() {
        return Err(Error::Invalid);
    }
    source.with_private_file(
        install.private,
        install.operation,
        work,
        |file, digest, work| {
            let text = match read_text(file, digest, length, work) {
                Ok(text) => text,
                Err(error) => return Err(storage_from_package(error)),
            };
            let request = Request {
                operation_id: install.operation.to_owned(),
                base: install.base.clone(),
                destination: Destination::Accepted,
                changes: vec![Change {
                    path: install.path.to_owned(),
                    text: Some(text),
                }],
            };
            let expected = [Expected {
                path: install.path.to_owned(),
                version: install.expected.clone(),
            }];
            let prepared = store.prepare(history, &request, &expected, scope)?;
            if prepared.newly_prepared {
                store.install(history, install.operation, scope)
            } else {
                Ok(prepared)
            }
        },
    )
}

/// Verify a held bundle. Objects are not admitted. Refs, config, hooks, remotes, and trust stay untouched.
pub fn verify_inert_bundle(
    source: &mut VerifiedSource,
    history: &History,
    operation: &str,
    private: &Path,
    claim: &Claim,
    work: &mut Work<'_>,
) -> Result<OwnerHandoff<Result<BundleReport, BundleError>>, Error> {
    native_operation(operation)?;
    source.with_private_file(private, operation, work, |file, digest, _work| {
        history.verify_bundle(file, digest, claim)
    })
}

/// Reverify the held bundle and admit objects only.
/// [`BundleReport::objects_admitted`] does not activate a ref.
pub fn admit_inert_objects(
    source: &mut VerifiedSource,
    history: &History,
    operation: &str,
    private: &Path,
    claim: &Claim,
    work: &mut Work<'_>,
) -> Result<OwnerHandoff<Result<BundleReport, BundleError>>, Error> {
    native_operation(operation)?;
    source.with_private_file(private, operation, work, |file, digest, _work| {
        history.admit_bundle(file, digest, claim)
    })
}

/// Encode `members` in slice order through `export_epub_held_funded`.
/// The output file must already be an empty private inode. Logical names are not source paths.
/// A refusal keeps [`EpubOwnerError`] intact, including unsettled children and partial descriptors.
pub fn export_ordered_epub(
    members: &[HeldMember<'_>],
    output: &File,
    title: &str,
    include_media: bool,
    child_rss: Option<u64>,
    storage_bytes: u64,
    funding: &mut Funding<'_>,
) -> Result<Admitted<HeldEpub>, EpubOwnerError> {
    let encoded: Vec<EpubMember<'_>> = members
        .iter()
        .map(|member| EpubMember {
            source: HeldSource {
                file: member.file,
                expected_version: member.version,
            },
            logical_name: member.logical_name,
        })
        .collect();
    knowledge_processing::epub::export_epub_held_funded(
        &encoded,
        output,
        title,
        include_media,
        child_rss,
        storage_bytes,
        funding,
    )
    .map_err(retain_epub)
}

/// Borrowed store and history for [`SafeApply`].
/// An unsettled history child is parked here until [`Self::take_unsettled`].
pub struct StorageOwner<'a, P: WritePolicy> {
    store: &'a Store<P>,
    history: &'a History,
    scope: Scope<'a>,
    preflight: Preflight<'a>,
    private_root: &'a Path,
    unsettled: Option<knowledge_history::UnsettledChild>,
}

impl<'a, P: WritePolicy> StorageOwner<'a, P> {
    /// Borrow the caller's current store, history, scope, and a distinct preflight holding.
    pub fn new(
        store: &'a Store<P>,
        history: &'a History,
        scope: Scope<'a>,
        preflight: Preflight<'a>,
        private_root: &'a Path,
    ) -> Self {
        Self {
            store,
            history,
            scope,
            preflight,
            private_root,
            unsettled: None,
        }
    }

    /// Return the parked history child. The caller collects it. This method does not settle the claim.
    pub fn take_unsettled(&mut self) -> Option<knowledge_history::UnsettledChild> {
        self.unsettled.take()
    }

    fn storage_error(&mut self, error: knowledge_storage::Error) -> Result<ApplyOutcome, Error> {
        match error {
            knowledge_storage::Error::Conflict => Ok(ApplyOutcome::Conflict),
            knowledge_storage::Error::Budget => Err(Error::Quota),
            knowledge_storage::Error::Waiting(_) => Err(Error::Waiting),
            knowledge_storage::Error::Admission(error) => Err(admission_error(error)),
            knowledge_storage::Error::Invalid
            | knowledge_storage::Error::Context
            | knowledge_storage::Error::Unsupported => Err(Error::Invalid),
            knowledge_storage::Error::History(knowledge_history::Error::Unsettled(child)) => {
                self.unsettled = Some(child);
                Err(Error::Indeterminate)
            }
            _ => Err(Error::Indeterminate),
        }
    }
}

impl<P: WritePolicy> SafeApply for StorageOwner<'_, P> {
    fn apply_text_change(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        expected: Option<&ContentDigest>,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        let private = private_name(operation_id, path.as_str())?;
        let version = match expected {
            None => None,
            Some(digest) => match self.store.observe(path.as_str(), &self.scope) {
                Ok(Some(version)) if version.digest == digest.as_str() => Some(version),
                Ok(_) => return Ok(ApplyOutcome::Conflict),
                Err(error) => return self.storage_error(error),
            },
        };
        let handoff = install_verified_text(
            candidate,
            self.store,
            self.history,
            &TextInstall {
                operation: &private,
                path: path.as_str(),
                expected: version,
                base: None,
                private: self.private_root,
            },
            &self.scope,
            work,
        )?;
        classify_install(self, handoff)
    }

    fn install_new(
        &mut self,
        operation_id: &str,
        path: &RelativePath,
        candidate: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<ApplyOutcome, Error> {
        let private = private_name(operation_id, path.as_str())?;
        let handoff = import_verified_source(
            candidate,
            self.store,
            &private,
            path.as_str(),
            self.private_root,
            &self.preflight,
            work,
        )?;
        match handoff.owner {
            Ok(_) if handoff.source_revalidation.is_err() => Ok(ApplyOutcome::Indeterminate),
            Ok(_) => Ok(ApplyOutcome::Saved),
            Err(error) => {
                let _ = handoff.source_revalidation;
                self.storage_error(error)
            }
        }
    }
}

/// Borrowed history for [`GitTransactions`].
/// Bundle admission does not move a ref. An unsettled child stays here until [`Self::take_unsettled`].
pub struct HistoryOwner<'a> {
    history: &'a History,
    claim: &'a Claim,
    private_root: &'a Path,
    unsettled: Option<knowledge_history::UnsettledChild>,
}

impl<'a> HistoryOwner<'a> {
    /// Borrow the caller's history and the same claim that owns its Git children.
    pub fn new(history: &'a History, claim: &'a Claim, private_root: &'a Path) -> Self {
        Self {
            history,
            claim,
            private_root,
            unsettled: None,
        }
    }

    /// Return the parked history child. The caller collects it. This method does not settle the claim.
    pub fn take_unsettled(&mut self) -> Option<knowledge_history::UnsettledChild> {
        self.unsettled.take()
    }
}

impl GitTransactions for HistoryOwner<'_> {
    fn admit_inert_bundle(
        &mut self,
        operation_id: &str,
        bundle: &mut VerifiedSource,
        work: &mut Work<'_>,
    ) -> Result<GitAdmit, Error> {
        let private = private_name(operation_id, "bundle")?;
        let handoff = admit_inert_objects(
            bundle,
            self.history,
            &private,
            self.private_root,
            self.claim,
            work,
        )?;
        match handoff.owner {
            Ok(report) if report.objects_admitted && handoff.source_revalidation.is_ok() => {
                Ok(GitAdmit::Inert)
            }
            Ok(_) => Ok(GitAdmit::Refused),
            Err(BundleError::Changed) => Err(Error::Drift),
            Err(BundleError::Integrity) => Err(Error::Checksum),
            Err(BundleError::History(knowledge_history::Error::Unsettled(child))) => {
                self.unsettled = Some(child);
                Err(Error::Indeterminate)
            }
            Err(BundleError::History(_)) => Err(Error::Indeterminate),
        }
    }
}

fn classify_install<P: WritePolicy>(
    owner: &mut StorageOwner<'_, P>,
    handoff: OwnerHandoff<Result<Inspection, knowledge_storage::Error>>,
) -> Result<ApplyOutcome, Error> {
    match handoff.owner {
        Ok(inspection) => Ok(match inspection.phase {
            Phase::FilesInstalled | Phase::SavedLocal if handoff.source_revalidation.is_ok() => {
                ApplyOutcome::Saved
            }
            Phase::Conflict => ApplyOutcome::Conflict,
            _ => ApplyOutcome::Indeterminate,
        }),
        Err(error) => {
            let _ = handoff.source_revalidation;
            owner.storage_error(error)
        }
    }
}

fn retain_epub(error: anyhow::Error) -> EpubOwnerError {
    match error.downcast::<PartialOutputs>() {
        Ok(partial) => EpubOwnerError::Partial(partial),
        Err(error) => match error.downcast::<knowledge_processing::worker::UnsettledChild>() {
            Ok(child) => EpubOwnerError::Unsettled(child),
            Err(error) => match error.downcast::<AdmitError>() {
                Ok(admission) => EpubOwnerError::Admission(admission),
                Err(error) => EpubOwnerError::Settled(error),
            },
        },
    }
}

/// Bytes kept with the text for its digest encoding and the request records.
const REQUEST_PREPARATION: u64 = 4096;

/// Charge the read text once, because `String::from_utf8` moves that buffer into the request.
/// The digest hex and its `ContentDigest` stay beside the buffer, so both encodings are included.
/// The operation, both path copies, the optional base, and the optional version digest stay live with that text.
/// Owner preparation is the fixed record envelope that `prepare` keeps with those values.
fn preparation_peak(length: u64, install: &TextInstall<'_>) -> Result<u64, Error> {
    let mut peak = length.checked_add(128).ok_or(Error::TooLarge)?;
    for len in [
        install.operation.len(),
        install.path.len(),
        install.path.len(),
        install.base.as_ref().map_or(0, |base| base.as_str().len()),
        install
            .expected
            .as_ref()
            .map_or(0, |version| version.digest.len()),
    ] {
        peak = peak
            .checked_add(u64::try_from(len).map_err(|_| Error::TooLarge)?)
            .ok_or(Error::TooLarge)?;
    }
    peak.checked_add(REQUEST_PREPARATION).ok_or(Error::TooLarge)
}

/// Fill the text buffer from the length already admitted by `preparation_peak`.
/// The caller charged that peak. This function does not charge again and does not stat the descriptor.
fn read_text(
    file: &File,
    digest: &ContentDigest,
    length: u64,
    work: &mut Work<'_>,
) -> Result<String, Error> {
    if length > work.bounds.quotas.max_entry_expanded {
        return Err(Error::Quota);
    }
    let mut bytes = Vec::new();
    let size = usize::try_from(length).map_err(|_| Error::TooLarge)?;
    bytes.try_reserve_exact(size).map_err(|_| Error::TooLarge)?;
    bytes.resize(size, 0);
    if length > 0 {
        file.read_exact_at(&mut bytes, 0).map_err(|_| Error::Io)?;
    }
    crate::note_read(work, length)?;
    let actual = crate::ContentDigest::new(crate::archive::hex(&sha2::Sha256::digest(&bytes)))
        .map_err(|_| Error::Invalid)?;
    if actual != *digest {
        return Err(Error::Checksum);
    }
    String::from_utf8(bytes).map_err(|_| Error::Invalid)
}

fn storage_from_package(error: Error) -> knowledge_storage::Error {
    match error {
        Error::Quota | Error::TooLarge => knowledge_storage::Error::Budget,
        Error::Waiting => {
            knowledge_storage::Error::Waiting(knowledge_core::admission::Waiting::Memory)
        }
        Error::Io => knowledge_storage::Error::Io,
        _ => knowledge_storage::Error::Invalid,
    }
}

fn private_name(operation: &str, path: &str) -> Result<String, Error> {
    let mut name = String::new();
    name.push_str(operation);
    name.push('-');
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || byte == b'_' {
            name.push(byte as char);
        } else {
            name.push('-');
        }
    }
    if name.len() > 96 {
        name.truncate(96);
    }
    while name.ends_with('-') {
        name.pop();
    }
    native_operation(&name)?;
    Ok(name)
}
