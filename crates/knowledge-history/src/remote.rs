//! Explicit native-selected transport, inert quarantine and expected-lease Git publication.
//! Imported URL/ref/attribution/config data cannot construct a live native consent.
//! Methods return fixed Error classifications, preserving UnsettledChild ownership; refusals retain candidates and uncertain pushes require inspection.
#![deny(missing_docs)]
use super::*;
use knowledge_core::data::ContentDigest;

/// Effects explicitly approved by the current native owner, never decoded from portable data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RemoteEffects {
    /// Inspect selected remote objects without publication permission.
    FetchOnly,
    /// Fetch plus separately intended, expected-lease publication to this selected ref only.
    FetchAndPush,
}
/// A selected local transport and live native effect capability; no Serialize/Deserialize implementation.
#[derive(Clone)]
pub struct RemoteConsent {
    /// Actual live native owner, separate from attribution.
    pub(crate) owner: NativeHistoryOwner,
    /// Exact explicitly selected local endpoint.
    target: PathBuf,
    /// Native endpoint directory identity.
    identity: Identity,
    /// Checked inert config version, not executable imported config.
    config: String,
    /// Single approved ordinary branch.
    reference: String,
    /// Upper bound on the selected native effects.
    effects: RemoteEffects,
}
/// Separate native human intention bound to the complete displayed integration tuple.
/// This cannot be deserialized or reconstructed from signed commit attribution.
pub struct RemoteConfirmation {
    /// Live decision-making native owner.
    owner: NativeHistoryOwner,
    /// Exact selected endpoint/ref/effects fingerprint.
    endpoint: String,
    /// Complete displayed immutable proposal/base/diff facts.
    tuple: RemotePull,
}
/// Native human intention for exactly one candidate/expected-remote-ref push.
pub struct RemotePushApproval {
    /// Live decision-making native owner.
    owner: NativeHistoryOwner,
    /// Exact endpoint/ref/effects scope.
    endpoint: String,
    /// One approved idempotent operation.
    operation_id: String,
    /// Exact local commit approved for publication.
    candidate: Oid,
    /// Exact expected remote ref, or explicitly approved creation.
    expected: Option<Oid>,
}
/// Relationship of actual fetched and accepted commits, not authority to install either tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RemoteRelation {
    /// No accepted local predecessor exists.
    Initial,
    /// Both actual heads are identical.
    Equal,
    /// The remote extends accepted ancestry.
    FastForward,
    /// Accepted ancestry extends the remote.
    LocalAhead,
    /// Neither head contains the other; preserve both as an inspected proposal.
    Diverged,
}
/// Retained actual remote proposal, readable through the ordinary proposal/version API.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RemotePull {
    /// Original fetch operation identity, retained across retries.
    pub operation_id: String,
    /// Qualified proposal retaining the actual fetched objects.
    pub proposal_id: String,
    /// Accepted version observed for this complete comparison.
    pub accepted: Option<Oid>,
    /// Actual fetched immutable commit, with original metadata/signature.
    pub fetched: Oid,
    /// Actual bounded Git ancestry classification.
    pub relation: RemoteRelation,
    /// SHA-256 of the complete displayed patch.
    pub diff_digest: String,
    /// Every affected selected text path.
    pub affected_paths: Vec<String>,
}
/// Actual push inspection; a refusal never deletes the local candidate or a remote writer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RemotePush {
    /// Actual selected remote publication proved and durably receipted.
    Published(Oid),
    /// Actual ref/ancestry/receive refusal; preserve the candidate and observed other writer.
    Refused {
        /// Retained local commit.
        candidate: Oid,
        /// Actual remote head observed after refusal, or an absent ref.
        remote: Option<Oid>,
    },
    /// Controlled effects may have occurred; inspection is required before any explicit new attempt.
    Indeterminate {
        /// Retained exact intended local commit.
        candidate: Oid,
    },
}
/// Immutable local recipe for uncertain pushes; completed facts have a separate immutable receipt.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PushIntent {
    /// Complete selected scope/candidate/lease digest.
    fingerprint: String,
    /// Exact pinned outbound commit.
    candidate: Oid,
    /// Exact pre-effect remote expectation.
    expected: Option<Oid>,
}
/// One immutable fetched-proposal recipe, independent of subsequent accepted-head movement.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct PullIntent {
    /// Selected native endpoint/ref/effect scope, without path or credentials.
    fingerprint: String,
    /// Actual immutable fetched-proposal facts.
    result: RemotePull,
}

impl NativeHistoryOwner {
    /// Bind a real native human confirmation to the exact displayed proposal/base/full diff.
    /// Only the trusted interactive owner path may invoke this; it is not exposed to a model or a schedule.
    pub fn confirm_remote(
        &self,
        consent: &RemoteConsent,
        tuple: RemotePull,
    ) -> Result<RemoteConfirmation, Error> {
        if !std::sync::Arc::ptr_eq(&self.selected, &consent.owner.selected) {
            return Err(Error::Denied);
        }
        self.selected.check()?;
        Ok(RemoteConfirmation {
            owner: self.clone(),
            endpoint: consent.fingerprint(),
            tuple,
        })
    }
    /// Bind native push intention to one exact operation, candidate and expected remote version.
    /// A generic selected URL/ref or cryptographic known signer is insufficient.
    pub fn approve_push(
        &self,
        consent: &RemoteConsent,
        operation_id: &str,
        candidate: &Oid,
        expected: Option<&Oid>,
    ) -> Result<RemotePushApproval, Error> {
        validate_id(operation_id)?;
        if !std::sync::Arc::ptr_eq(&self.selected, &consent.owner.selected)
            || consent.effects != RemoteEffects::FetchAndPush
        {
            return Err(Error::Denied);
        }
        self.selected.check()?;
        Ok(RemotePushApproval {
            owner: self.clone(),
            endpoint: consent.fingerprint(),
            operation_id: operation_id.into(),
            candidate: candidate.clone(),
            expected: expected.cloned(),
        })
    }
    /// Record the native user's explicit selected local bare transport/ref/effects.
    /// The host supplies this ONLY after current owner consent, not from SourceOwnerContext or an archive URL.
    /// Ordinary local bare servers use no personal config, agent, credential helper or network.
    pub fn approve_local_remote(
        &self,
        history: &History,
        target: &Path,
        reference: &str,
        effects: RemoteEffects,
        work: HistoryWork<'_>,
    ) -> Result<RemoteConsent, Error> {
        let call = work.call(history, self)?;
        let target = selected_directory(target)?;
        if target == history.repo
            || target.starts_with(&history.home)
            || history.home.starts_with(&target)
        {
            return Err(Error::Invalid);
        }
        validate_remote_ref(reference)?;
        let scratch = history.scratch(&call)?;
        run(
            &history.repo,
            scratch.path(),
            &["check-ref-format", reference],
            b"",
            None,
            &call,
            (None, &history.runtime),
        )?;
        let config = local_config(&target, &call)?;
        let consent = RemoteConsent {
            owner: self.clone(),
            identity: identity(&target)?,
            target,
            config,
            reference: reference.into(),
            effects,
        };
        consent.check(history, false, &call)?;
        Ok(consent)
    }
}
impl RemoteConsent {
    /// Revalidate live selection, exact selected endpoint identity and inert server configuration.
    pub(crate) fn check(&self, history: &History, push: bool, call: &Call) -> Result<(), Error> {
        call.check()?;
        self.owner.check(history)?;
        if push && self.effects != RemoteEffects::FetchAndPush {
            return Err(Error::Denied);
        }
        if identity(&self.target)? != self.identity
            || local_config(&self.target, call)? != self.config
        {
            return Err(Error::ContextMismatch);
        }
        let reference = self.target.join(&self.reference);
        if fs::symlink_metadata(&reference).is_ok() {
            let bytes = bounded_read(&reference, 128)?;
            let text = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid)?;
            // A symbolic server ref must not redirect an approved push to another branch.
            Oid::parse(text.strip_suffix('\n').ok_or(Error::Invalid)?)?;
        }
        Ok(())
    }
    /// Bind durable recipes to the selected endpoint/ref/effects without storing credentials or paths.
    pub(crate) fn fingerprint(&self) -> String {
        digest(
            format!(
                "{:?}:{}:{}:{:?}",
                self.identity, self.config, self.reference, self.effects
            )
            .as_bytes(),
        )
    }
    /// Build only the already approved file transport, with bounded packs and disabled remote executable hooks.
    fn command(&self, repo: &Path, scratch: &Path, program: &str) -> Command {
        let mut command = git_command(repo, scratch, program);
        command.args([
            "-c",
            "protocol.file.allow=always",
            "-c",
            "fetch.unpackLimit=0",
            "-c",
            "transfer.unpackLimit=0",
            "-c",
            "fetch.fsckObjects=true",
            "-c",
            "transfer.fsckObjects=true",
            "-c",
            "pack.threads=1",
            "-c",
            "pack.windowMemory=1m",
            "-c",
            "core.deltaBaseCacheLimit=1m",
            "-c",
            "core.packedGitLimit=1m",
        ]);
        command
    }
    /// Inspect only the exact selected advertised ref, with complete capped native output.
    fn advertised(
        &self,
        history: &History,
        scratch: &Path,
        call: &Call,
    ) -> Result<Option<Oid>, Error> {
        self.check(history, false, call)?;
        let mut command = self.command(&history.repo, scratch, history.runtime.git_program(call)?);
        command.args([
            "ls-remote",
            "--refs",
            "--",
            self.target.to_str().ok_or(Error::Invalid)?,
            &self.reference,
        ]);
        let output = history.runtime.execute(command, b"", call)?;
        if !output.status.success() {
            return Err(Error::Git);
        }
        self.check(history, false, call)?;
        if output.stdout.is_empty() {
            return Ok(None);
        }
        let text = std::str::from_utf8(&output.stdout).map_err(|_| Error::Recovery)?;
        let mut lines = text.lines();
        let (oid, name) = lines
            .next()
            .and_then(|l| l.split_once('\t'))
            .ok_or(Error::Recovery)?;
        if name != self.reference || lines.next().is_some() {
            return Err(Error::Recovery);
        }
        call.account_files(1)?;
        Ok(Some(Oid::parse(oid)?))
    }
}
impl History {
    /// Fetch actual selected objects into inert private quarantine, then admit a retained proposal only.
    /// Never checks out or installs home bytes, activates remote config, or advances accepted history.
    /// Duplicate completed delivery returns the original proposal, even after accepted/ref movement.
    pub fn pull_remote(
        &self,
        operation_id: &str,
        consent: &RemoteConsent,
        work: HistoryWork<'_>,
    ) -> Result<RemotePull, Error> {
        let call = work.call(self, &consent.owner)?;
        self.pull_remote_call(operation_id, consent, &call)
    }
    /// Reuse the original guarded scheduling call and its complete work budget.
    pub(crate) fn pull_remote_call(
        &self,
        operation_id: &str,
        consent: &RemoteConsent,
        call: &Call,
    ) -> Result<RemotePull, Error> {
        validate_id(operation_id)?;
        let _lock = self.lock(call)?;
        consent.check(self, false, call)?;
        let fingerprint = consent.fingerprint();
        let path = self.remote_path(operation_id, "pull")?;
        let scratch = self.scratch(call)?;
        if path.exists() {
            let retained = self.read_pull(operation_id)?;
            if retained.fingerprint != fingerprint {
                return Err(Error::OperationMismatch);
            }
            self.publish_pull(&retained.result, scratch.path(), call)?;
            return Ok(retained.result);
        }
        let fetched = consent
            .advertised(self, scratch.path(), call)?
            .ok_or(Error::Conflict)?;
        let accepted = self.reference(ACCEPTED, scratch.path(), call)?;
        let quarantine = scratch.path().join("quarantine.git");
        fs::create_dir(&quarantine)?;
        run(
            &quarantine,
            scratch.path(),
            &["init", "--bare", "--object-format=sha1", "--template=", "."],
            b"",
            None,
            call,
            (None, &self.runtime),
        )?;
        fs::write(quarantine.join("config"), CONFIG)?;
        consent.check(self, false, call)?;
        let refspec = format!(
            "{}:refs/heads/omni-proposals/{operation_id}",
            consent.reference
        );
        let mut command =
            consent.command(&quarantine, scratch.path(), self.runtime.git_program(call)?);
        command.args(["fetch", "--no-tags", "--no-write-fetch-head", "--no-recurse-submodules", "--no-auto-maintenance",
            "--upload-pack=/usr/bin/git -c core.hooksPath=/dev/null -c core.fsmonitor=false -c gc.auto=0 -c maintenance.auto=false -c pack.threads=1 -c pack.windowMemory=1m upload-pack",
            "--", consent.target.to_str().ok_or(Error::Invalid)?, &refspec]);
        let output = self.runtime.execute(command, b"", call)?;
        if !output.status.success() {
            return Err(Error::Git);
        }
        consent.check(self, false, call)?;
        let proposal = format!("refs/heads/omni-proposals/{operation_id}");
        let actual = oid_output(run(
            &quarantine,
            scratch.path(),
            &["rev-parse", "--verify", &proposal],
            b"",
            None,
            call,
            (None, &self.runtime),
        )?)?;
        if actual != fetched
            || consent.advertised(self, scratch.path(), call)? != Some(fetched.clone())
        {
            return Err(Error::Conflict);
        }
        self.bound_object_graph(&quarantine, &fetched, None, scratch.path(), call)?;
        self.validate_remote_tree(&quarantine, &fetched, scratch.path(), call)?;
        // The existing exact-stream bundle owner performs pack integrity and admission, not a second Git engine.
        let bundle = scratch.path().join("fetched.bundle");
        run(
            &quarantine,
            scratch.path(),
            &[
                "bundle",
                "create",
                bundle.to_str().ok_or(Error::Invalid)?,
                &proposal,
            ],
            b"",
            None,
            call,
            (None, &self.runtime),
        )?;
        let file = File::open(&bundle)?;
        let bytes = bounded_read(&bundle, call.limit)?;
        call.account_bytes(bytes.len())?;
        let version = ContentDigest::new(digest(&bytes)).map_err(|_| Error::Recovery)?;
        drop(bytes);
        self.bundle_call(&file, &version, true, call)
            .map_err(bundle_error)?;
        let tree = self.base_tree(Some(&fetched), scratch.path(), call)?;
        let diff = self.compare_tree(accepted.as_ref(), &tree, scratch.path(), call)?;
        let relation = match &accepted {
            None => RemoteRelation::Initial,
            Some(base) if base == &fetched => RemoteRelation::Equal,
            Some(base) if self.ancestor(base, &fetched, scratch.path(), call)? => {
                RemoteRelation::FastForward
            }
            Some(base) if self.ancestor(&fetched, base, scratch.path(), call)? => {
                RemoteRelation::LocalAhead
            }
            Some(_) => RemoteRelation::Diverged,
        };
        let result = RemotePull {
            operation_id: operation_id.into(),
            proposal_id: operation_id.into(),
            accepted,
            fetched,
            relation,
            diff_digest: diff.diff_digest,
            affected_paths: diff.affected_paths,
        };
        consent.check(self, false, call)?;
        persist(
            &path,
            &PullIntent {
                fingerprint,
                result: result.clone(),
            },
        )?;
        self.publish_pull(&result, scratch.path(), call)?;
        Ok(result)
    }

    /// Push one qualified accepted candidate with an exact remote lease and ancestry proof.
    /// Refusal/timeout preserves the candidate; a restarted uncertain attempt inspects, never blindly replays.
    /// A completed operation stays idempotent even if another remote writer subsequently advances.
    pub fn push_remote(
        &self,
        approval: &RemotePushApproval,
        consent: &RemoteConsent,
        work: HistoryWork<'_>,
    ) -> Result<RemotePush, Error> {
        let call = work.call(self, &consent.owner)?;
        self.push_remote_call(approval, consent, &call)
    }
    /// Reuse the original scheduling call; push cannot obtain a renewed memory/I/O/deadline envelope.
    pub(crate) fn push_remote_call(
        &self,
        approval: &RemotePushApproval,
        consent: &RemoteConsent,
        call: &Call,
    ) -> Result<RemotePush, Error> {
        approval.owner.check(self)?;
        if !std::sync::Arc::ptr_eq(&approval.owner.selected, &consent.owner.selected)
            || approval.endpoint != consent.fingerprint()
        {
            return Err(Error::Denied);
        }
        let operation_id = approval.operation_id.as_str();
        let candidate = &approval.candidate;
        let expected = approval.expected.as_ref();
        validate_id(operation_id)?;
        let _lock = self.lock(call)?;
        consent.check(self, true, call)?;
        let fingerprint = digest(
            &serde_json::to_vec(&(consent.fingerprint(), candidate, expected))
                .map_err(|_| Error::Invalid)?,
        );
        let path = self.remote_path(operation_id, "push")?;
        let complete = self.remote_path(operation_id, "pushed")?;
        let scratch = self.scratch(call)?;
        if path.exists() {
            let intent: PushIntent = load(&path)?;
            if intent.fingerprint != fingerprint
                || &intent.candidate != candidate
                || intent.expected.as_ref() != expected
            {
                return Err(Error::OperationMismatch);
            }
            if complete.exists() {
                let proof: Oid = load(&complete)?;
                if &proof != candidate {
                    return Err(Error::Recovery);
                }
                return Ok(RemotePush::Published(proof));
            }
            let remote = consent.advertised(self, scratch.path(), call)?;
            if remote.as_ref() == Some(candidate) {
                persist(&complete, candidate)?;
                return Ok(RemotePush::Published(candidate.clone()));
            }
            return Ok(RemotePush::Indeterminate {
                candidate: candidate.clone(),
            });
        }
        let accepted = self
            .reference(ACCEPTED, scratch.path(), call)?
            .ok_or(Error::Conflict)?;
        if !self.ancestor(candidate, &accepted, scratch.path(), call)? {
            return Err(Error::Denied);
        }
        let remote = consent.advertised(self, scratch.path(), call)?;
        if remote.as_ref() != expected {
            return Ok(RemotePush::Refused {
                candidate: candidate.clone(),
                remote,
            });
        }
        if let Some(base) = expected {
            // A missing remote ancestor is a surfaced refusal, never a reason to force an unknown graph.
            match self.git(
                &["cat-file", "-e", base.as_str()],
                b"",
                scratch.path(),
                None,
                call,
            ) {
                Ok(_) => {}
                Err(Error::Git) => {
                    return Ok(RemotePush::Refused {
                        candidate: candidate.clone(),
                        remote,
                    });
                }
                Err(error) => return Err(error),
            }
            if !self.ancestor(base, candidate, scratch.path(), call)? {
                return Ok(RemotePush::Refused {
                    candidate: candidate.clone(),
                    remote,
                });
            }
        }
        self.bound_object_graph(&self.repo, candidate, expected, scratch.path(), call)?;
        // Pin the exact candidate before any controlled remote effect; neither cancellation nor offline failure deletes it.
        let pin = format!("refs/omni/remote-push/{operation_id}");
        let transaction = format!(
            "start\ncreate {pin} {}\nprepare\ncommit\n",
            candidate.as_str()
        );
        match self.reference(&pin, scratch.path(), call)? {
            Some(prior) if &prior == candidate => {}
            Some(_) => return Err(Error::Conflict),
            None => {
                self.git(
                    &["update-ref", "--stdin", "--no-deref"],
                    transaction.as_bytes(),
                    scratch.path(),
                    None,
                    call,
                )?;
            }
        }
        persist(
            &path,
            &PushIntent {
                fingerprint,
                candidate: candidate.clone(),
                expected: expected.cloned(),
            },
        )?;
        consent.check(self, true, call)?;
        let lease = format!(
            "--force-with-lease={}:{}",
            consent.reference,
            expected.map_or("", Oid::as_str)
        );
        let refspec = format!("{}:{}", candidate.as_str(), consent.reference);
        let mut command =
            consent.command(&self.repo, scratch.path(), self.runtime.git_program(call)?);
        command.args(["push", "--porcelain", "--no-verify", "--recurse-submodules=no", &lease,
            "--receive-pack=/usr/bin/git -c core.hooksPath=/dev/null -c core.fsmonitor=false -c receive.unpackLimit=0 -c receive.autogc=false -c gc.auto=0 -c maintenance.auto=false -c transfer.fsckObjects=true -c pack.threads=1 receive-pack",
            "--", consent.target.to_str().ok_or(Error::Invalid)?, &refspec]);
        let output = match self.runtime.execute(command, b"", call) {
            Ok(output) => output,
            Err(error @ Error::Unsettled(_)) => return Err(error),
            Err(_) => {
                return Ok(RemotePush::Indeterminate {
                    candidate: candidate.clone(),
                });
            }
        };
        if consent.check(self, true, call).is_err() {
            return Ok(RemotePush::Indeterminate {
                candidate: candidate.clone(),
            });
        }
        let remote = match consent.advertised(self, scratch.path(), call) {
            Ok(remote) => remote,
            Err(error @ Error::Unsettled(_)) => return Err(error),
            Err(_) => {
                return Ok(RemotePush::Indeterminate {
                    candidate: candidate.clone(),
                });
            }
        };
        if remote.as_ref() == Some(candidate) {
            persist(&complete, candidate)?;
            Ok(RemotePush::Published(candidate.clone()))
        } else if !output.status.success() {
            Ok(RemotePush::Refused {
                candidate: candidate.clone(),
                remote,
            })
        } else {
            Ok(RemotePush::Indeterminate {
                candidate: candidate.clone(),
            })
        }
    }

    /// Confirm only the exact displayed remote proposal using a separate live native human decision.
    /// The returned candidate still requires M1's actual safe installer before ordinary Git publication.
    pub fn prepare_remote_confirmation(
        &self,
        operation_id: &str,
        approval: &RemoteConfirmation,
        consent: &RemoteConsent,
        work: HistoryWork<'_>,
    ) -> Result<Prepared, Error> {
        approval.owner.check(self)?;
        if !std::sync::Arc::ptr_eq(&approval.owner.selected, &consent.owner.selected)
            || approval.endpoint != consent.fingerprint()
        {
            return Err(Error::Denied);
        }
        let displayed = &approval.tuple;
        let call = work.call(self, &consent.owner)?;
        consent.check(self, false, &call)?;
        let retained = self.read_pull(&displayed.proposal_id)?;
        if retained.fingerprint != consent.fingerprint() || retained.result != *displayed {
            return Err(Error::Conflict);
        }
        if let Some(base) = &displayed.accepted {
            return self.prepare_confirmation_call(
                operation_id,
                &Confirmation {
                    proposal_id: displayed.proposal_id.clone(),
                    base: base.clone(),
                    candidate: displayed.fetched.clone(),
                    diff_digest: displayed.diff_digest.clone(),
                },
                &call,
            );
        }
        // Initial working-copy continuation retains the original remote commit as a parent; it is never relabelled.
        validate_id(operation_id)?;
        if self.owner.agent.is_some() {
            return Err(Error::Denied);
        }
        let _lock = self.lock(&call)?;
        let fingerprint =
            digest(&serde_json::to_vec(&(operation_id, displayed)).map_err(|_| Error::Invalid)?);
        if let Some(intent) = self.existing(operation_id, &fingerprint)? {
            if matches!(
                self.inspect_intent(&intent, &call)?,
                State::Conflict(_) | State::Indeterminate(_)
            ) {
                return Err(Error::Conflict);
            }
            return self.finish_prepare(intent, &call);
        }
        let scratch = self.scratch(&call)?;
        let proposal_ref = format!("refs/heads/omni-proposals/{}", displayed.proposal_id);
        if self.reference(ACCEPTED, scratch.path(), &call)?.is_some()
            || self.reference(&proposal_ref, scratch.path(), &call)?
                != Some(displayed.fetched.clone())
        {
            return Err(Error::Conflict);
        }
        let tree = self.base_tree(Some(&displayed.fetched), scratch.path(), &call)?;
        let comparison = self.compare_tree(None, &tree, scratch.path(), &call)?;
        if comparison.diff_digest != displayed.diff_digest {
            return Err(Error::Conflict);
        }
        let mut intent = self.intent(
            operation_id,
            fingerprint,
            None,
            ACCEPTED.into(),
            vec![displayed.fetched.clone()],
            tree,
            comparison.affected_paths,
        )?;
        intent.proposal = Some(ConfirmedProposal {
            reference: proposal_ref,
            oid: displayed.fetched.clone(),
            initial: true,
        });
        if intent.prepared.unchanged {
            intent.adopted = Some(displayed.fetched.clone());
        }
        self.retain_tree(&intent, scratch.path(), &call)?;
        persist(&self.intent_path(operation_id), &intent)?;
        self.finish_prepare(intent, &call)
    }

    /// Bound all actual graph entries and uncompressed object bytes before admission or outbound pack generation.
    /// Native traversal/output/RSS are already bounded; no advertised size or compressed-pack ratio is trusted.
    fn bound_object_graph(
        &self,
        repo: &Path,
        candidate: &Oid,
        exclude: Option<&Oid>,
        scratch: &Path,
        call: &Call,
    ) -> Result<(), Error> {
        let exclusion = exclude.map(|oid| format!("^{}", oid.as_str()));
        let mut args = vec![
            "rev-list",
            "--objects",
            "--no-object-names",
            candidate.as_str(),
        ];
        if let Some(exclusion) = &exclusion {
            args.push(exclusion);
        }
        let objects = run(repo, scratch, &args, b"", None, call, (None, &self.runtime))?;
        let count = objects
            .split(|b| *b == b'\n')
            .filter(|s| !s.is_empty())
            .count();
        call.account_files(count as u64)?;
        let sizes = run(
            repo,
            scratch,
            &[
                "cat-file",
                "--batch-check=%(objectname) %(objecttype) %(objectsize)",
            ],
            &objects,
            None,
            call,
            (None, &self.runtime),
        )?;
        let mut bytes = 0u64;
        let mut checked = 0usize;
        for line in std::str::from_utf8(&sizes)
            .map_err(|_| Error::Recovery)?
            .lines()
        {
            let parts: Vec<_> = line.split(' ').collect();
            if parts.len() != 3 || !matches!(parts[1], "blob" | "tree" | "commit" | "tag") {
                return Err(Error::Invalid);
            }
            Oid::parse(parts[0])?;
            bytes = bytes
                .checked_add(parts[2].parse::<u64>().map_err(|_| Error::Recovery)?)
                .ok_or(Error::Capacity)?;
            if bytes > call.bytes_left.get() {
                return Err(Error::Capacity);
            }
            checked += 1;
        }
        if count != checked {
            return Err(Error::Recovery);
        }
        call.account_bytes(usize::try_from(bytes).map_err(|_| Error::Capacity)?)?;
        Ok(())
    }
    /// Validate the actual selected fetched tree as bounded regular UTF-8 text, before object admission/publication.
    fn validate_remote_tree(
        &self,
        quarantine: &Path,
        fetched: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<(), Error> {
        let listing = run(
            quarantine,
            scratch,
            &["ls-tree", "-r", "-z", fetched.as_str()],
            b"",
            None,
            call,
            (None, &self.runtime),
        )?;
        for entry in listing.split(|b| *b == 0).filter(|e| !e.is_empty()) {
            call.account_files(1)?;
            let (meta, path) = std::str::from_utf8(entry)
                .map_err(|_| Error::Invalid)?
                .split_once('\t')
                .ok_or(Error::Invalid)?;
            self.validate_path(path)?;
            let blob = meta.strip_prefix("100644 blob ").ok_or(Error::Invalid)?;
            let oid = Oid::parse(blob)?;
            let bytes = run(
                quarantine,
                scratch,
                &["cat-file", "blob", oid.as_str()],
                b"",
                None,
                call,
                (None, &self.runtime),
            )?;
            if bytes.contains(&0) || std::str::from_utf8(&bytes).is_err() {
                return Err(Error::Invalid);
            }
        }
        Ok(())
    }
    /// Prove scheduled native work from actual immutable refs and durable expected-effect receipts.
    pub(crate) fn remote_completion(
        &self,
        operation_id: &str,
        allow_push: bool,
        call: &Call,
    ) -> Result<(), Error> {
        let intent = self.read_pull(operation_id)?;
        let scratch = self.scratch(call)?;
        let receipt = format!("refs/omni/remote-pulls/{operation_id}");
        if self.reference(&receipt, scratch.path(), call)? != Some(intent.result.fetched.clone()) {
            return Err(Error::Recovery);
        }
        if allow_push && intent.result.relation == RemoteRelation::LocalAhead {
            let proof: Oid = load(&self.remote_path(operation_id, "pushed")?)?;
            if Some(proof) != intent.result.accepted {
                return Err(Error::Recovery);
            }
        }
        Ok(())
    }
    /// Install only a retained proposal ref, with a separate immutable receipt in the same Git CAS transaction.
    fn publish_pull(&self, result: &RemotePull, scratch: &Path, call: &Call) -> Result<(), Error> {
        let proposal = format!("refs/heads/omni-proposals/{}", result.proposal_id);
        let receipt = format!("refs/omni/remote-pulls/{}", result.operation_id);
        let prior = self.reference(&receipt, scratch, call)?;
        if prior == Some(result.fetched.clone()) {
            return Ok(());
        }
        if prior.is_some() {
            return Err(Error::Recovery);
        }
        let current = self.reference(&proposal, scratch, call)?;
        if current.is_some() && current != Some(result.fetched.clone()) {
            return Err(Error::Conflict);
        }
        let update = if current.is_some() {
            format!("verify {proposal} {}\n", result.fetched.as_str())
        } else {
            format!("create {proposal} {}\n", result.fetched.as_str())
        };
        let transaction = format!(
            "start\n{update}create {receipt} {}\nprepare\ncommit\n",
            result.fetched.as_str()
        );
        self.git(
            &["update-ref", "--stdin", "--no-deref"],
            transaction.as_bytes(),
            scratch,
            None,
            call,
        )?;
        Ok(())
    }
    /// Bounded actual Git ancestry proof; native errors remain errors, never false permission.
    fn ancestor(
        &self,
        base: &Oid,
        candidate: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<bool, Error> {
        let mut command = git_command(&self.repo, scratch, self.runtime.git_program(call)?);
        command.args([
            "merge-base",
            "--is-ancestor",
            base.as_str(),
            candidate.as_str(),
        ]);
        let output = self.runtime.execute(command, b"", call)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => Err(Error::Git),
        }
    }
    /// Validate every retained argument before it can reach a ref transaction, path or native execution.
    fn read_pull(&self, operation_id: &str) -> Result<PullIntent, Error> {
        let intent: PullIntent = load(&self.remote_path(operation_id, "pull")?)?;
        if intent.result.operation_id != operation_id
            || intent.result.proposal_id != operation_id
            || !valid_digest(&intent.fingerprint)
            || !valid_digest(&intent.result.diff_digest)
        {
            return Err(Error::Recovery);
        }
        validate_id(&intent.result.proposal_id)?;
        for path in &intent.result.affected_paths {
            self.validate_path(path)?;
        }
        Ok(intent)
    }
    /// Resolve only a validated local receipt path; no remote configuration or URL is persisted.
    fn remote_path(&self, operation_id: &str, kind: &str) -> Result<PathBuf, Error> {
        validate_id(operation_id)?;
        Ok(self
            .repo
            .join("omni-state/operations")
            .join(format!("{operation_id}.{kind}.json")))
    }
}
/// Propagate actual native unknown-owner errors through bundle admission, never a scope waiver.
fn bundle_error(error: BundleError) -> Error {
    match error {
        BundleError::History(error) => error,
        BundleError::Changed => Error::Conflict,
        BundleError::Integrity => Error::Invalid,
    }
}
/// Restrict server refs to one qualified ordinary branch, without wildcard/revision/option syntax.
fn validate_remote_ref(reference: &str) -> Result<(), Error> {
    let leaf = reference
        .strip_prefix("refs/heads/")
        .ok_or(Error::Invalid)?;
    if leaf.is_empty()
        || reference.len() > 256
        || !leaf
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_/".contains(&b))
    {
        return Err(Error::Invalid);
    }
    for part in leaf.split('/') {
        validate_id(part)?;
    }
    Ok(())
}
/// Inspect only inert default bare-server config; no includes, hooks/helpers, transport rewrites or alternates run.
/// This is intentionally not imported repository configuration admission.
fn local_config(target: &Path, call: &Call) -> Result<String, Error> {
    selected_directory(target)?;
    for path in ["objects", "objects/info", "objects/pack", "refs"] {
        selected_directory(&target.join(path))?;
    }
    for path in [
        "objects/info/alternates",
        "objects/info/http-alternates",
        "commondir",
        "config.worktree",
        "shallow",
    ] {
        if fs::symlink_metadata(target.join(path)).is_ok() {
            return Err(Error::Invalid);
        }
    }
    // Git must not follow escaping object/ref symlinks or special files inside a selected local server.
    let mut stack = vec![target.join("objects"), target.join("refs")];
    let mut entries = 0u64;
    while let Some(path) = stack.pop() {
        entries = entries.checked_add(1).ok_or(Error::Capacity)?;
        if entries > call.files.saturating_add(64) {
            return Err(Error::Capacity);
        }
        let meta = fs::symlink_metadata(&path)?;
        if meta.is_dir() {
            for entry in fs::read_dir(path)? {
                if stack.len() as u64 > call.files.saturating_add(64) {
                    return Err(Error::Capacity);
                }
                stack.push(entry?.path());
            }
        } else if !meta.is_file() || meta.file_type().is_symlink() {
            return Err(Error::Invalid);
        }
    }
    for name in ["HEAD", "packed-refs"] {
        if fs::symlink_metadata(target.join(name))
            .is_ok_and(|m| !m.is_file() || m.file_type().is_symlink())
        {
            return Err(Error::Invalid);
        }
    }
    let bytes = bounded_read(&target.join("config"), call.limit)?;
    let text = std::str::from_utf8(&bytes).map_err(|_| Error::Invalid)?;
    let mut section = "";
    let mut bare = false;
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        if line.starts_with('[') {
            section = match line {
                "[core]" => "core",
                "[gc]" => "gc",
                _ => return Err(Error::Invalid),
            };
            continue;
        }
        let (name, value) = line.split_once('=').ok_or(Error::Invalid)?;
        match (section, name.trim(), value.trim()) {
            ("core", "repositoryformatversion", "0")
            | ("core", "filemode", "true" | "false")
            | ("core", "logallrefupdates", "false" | "true")
            | ("gc", "auto", "0") => {}
            ("core", "bare", "true") => bare = true,
            _ => return Err(Error::Invalid),
        }
    }
    if !bare {
        return Err(Error::Invalid);
    }
    Ok(digest(&bytes))
}
