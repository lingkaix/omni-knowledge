//! Qualified proposal and immutable-history reads on the existing native Git transaction owner.

use super::*;

/// Actual Git identity and clock recorded in one commit, not caller authority.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitPerson {
    /// Recorded display name.
    pub name: String,
    /// Recorded email.
    pub email: String,
    /// Recorded Unix seconds, including historical dates before the epoch.
    pub seconds: i64,
    /// Original numeric UTC offset.
    pub timezone: String,
}

/// Signature presence is metadata only. Verification is a separate native cryptographic operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SignatureFormat {
    /// No signature header exists.
    Unsigned,
    /// An OpenSSH signature is present; this alone does not say it is valid or known.
    Ssh,
    /// Another signature format is retained, not mislabelled as a verified SSH signature.
    Other,
}

/// Recorded operation/agent facts; imported trailers remain attribution claims, not authentication.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordedAttribution {
    /// Recorded operation identity, if it has a valid bounded spelling.
    pub operation_id: Option<String>,
    /// Agent-on-behalf-of-user display fact.
    pub agent: Option<String>,
    /// Tool fact, without raw arguments.
    pub tool: Option<String>,
    /// Session fact, without a session URL or query.
    pub session: Option<String>,
    /// External observation origin, when recorded explicitly.
    pub origin: Option<String>,
    /// Recorded stable-observation evidence IDs, never source/read authority.
    pub observation_ids: Vec<String>,
}

/// Actual immutable commit metadata under the admitted history context.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitInfo {
    /// Immutable version, never a replacement for current source admission.
    pub version: Oid,
    /// Actual complete tree identity.
    pub tree: Oid,
    /// Actual ordered parents; remote author/signature history is not relabelled.
    pub parents: Vec<Oid>,
    /// Actual author, separate from committer and agent claims.
    pub author: GitPerson,
    /// Actual committer and clock.
    pub committer: GitPerson,
    /// Bounded actual message, not a newly invented summary.
    pub message: String,
    /// Complete selected affected paths against the first parent (or empty tree for a root commit).
    pub affected_paths: Vec<String>,
    /// Actual final-paragraph attribution fields.
    pub attribution: RecordedAttribution,
    /// Signature presence, explicitly not cryptographic verification.
    pub signature: SignatureFormat,
}

/// One current qualified app proposal, not a general branch handle or approval grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Proposal {
    /// Validated app proposal ID.
    pub proposal_id: String,
    /// Original first parent; initial proposals may have no accepted predecessor.
    pub original_base: Option<Oid>,
    /// Exact current proposal head, used again by confirmation/publication CAS.
    pub candidate: Oid,
    /// Actual author/committer/signature-presence facts.
    pub commit: CommitInfo,
}

/// Finite proposal page; continuation is an exclusive app proposal ID, not authority.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalPage {
    /// Qualified entries only; unrelated branches and internal recovery/cache refs never appear.
    pub entries: Vec<Proposal>,
    /// Pass as after to continue; None means this observed page had no further entry.
    pub next: Option<String>,
}

/// Exact currently inspected proposal/base/diff tuple, with a complete bounded patch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProposalDiff {
    /// Qualified exact proposal snapshot.
    pub proposal: Proposal,
    /// Actual accepted head used for the displayed complete diff.
    pub expected_base: Option<Oid>,
    /// Full patch/digest/affected-path binding, never a truncated confirmation.
    pub comparison: Comparison,
}

/// Finite first-parent accepted history page; continuation is requalified on each call.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommitPage {
    /// Actual ordered commits, newest first.
    pub entries: Vec<CommitInfo>,
    /// Next actual first parent; passing it does not authorize another repository or branch.
    pub next: Option<Oid>,
}

impl History {
    /// Enumerate only direct qualified app proposal refs, under the current native binding and finite claim.
    /// Continuation uses native Git's start-after facility; no imported branch-management surface is exposed.
    pub fn proposals(
        &self,
        after: Option<&str>,
        limit: usize,
        claim: &Claim,
    ) -> Result<ProposalPage, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        check_limit(limit, &call)?;
        if let Some(after) = after {
            validate_id(after)?;
        }
        let scratch = self.scratch()?;
        let count = format!("--count={}", limit.checked_add(1).ok_or(Error::Capacity)?);
        // Native namespace-scoped glob union implements an exclusive ASCII cursor even on older system Git.
        // No --start-after feature dependency, unbounded prefix scan, private-ref output or local branch index.
        let prefix = "refs/heads/omni-proposals/";
        let mut patterns = Vec::new();
        if let Some(after) = after {
            const ALPHABET: &[u8] =
                b"-0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ_abcdefghijklmnopqrstuvwxyz";
            for (index, byte) in after.bytes().enumerate() {
                let greater: String = ALPHABET
                    .iter()
                    .copied()
                    .filter(|v| *v > byte)
                    .map(char::from)
                    .collect();
                if !greater.is_empty() {
                    patterns.push(format!("{prefix}{}[{greater}]*", &after[..index]));
                }
            }
            patterns.push(format!("{prefix}{after}?*")); // Never invoke native enumeration with no namespace pattern.
        } else {
            patterns.push(prefix.to_owned());
        }
        if patterns.iter().map(String::len).sum::<usize>() > call.limit {
            return Err(Error::Capacity);
        }
        let mut args = vec![
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname) %(objectname) %(symref)",
            &count,
        ];
        args.extend(patterns.iter().map(String::as_str));
        let listing = self.git(&args, b"", scratch.path(), None, &call)?;
        let text = std::str::from_utf8(&listing).map_err(|_| Error::Recovery)?;
        let mut names = Vec::new();
        for line in text.lines() {
            call.account_files(1)?;
            let fields: Vec<_> = line.split(' ').collect();
            if fields.len() != 3 || !fields[2].is_empty() {
                return Err(Error::Recovery);
            }
            let id = fields[0]
                .strip_prefix("refs/heads/omni-proposals/")
                .ok_or(Error::Recovery)?;
            validate_id(id).map_err(|_| Error::Recovery)?;
            names.push((id.to_owned(), Oid::parse(fields[1])?));
        }
        let more = names.len() > limit;
        names.truncate(limit);
        let mut entries = Vec::with_capacity(names.len());
        for (id, candidate) in names {
            let commit = self.commit_info(&candidate, scratch.path(), &call)?;
            self.base_tree(Some(&candidate), scratch.path(), &call)?;
            entries.push(Proposal {
                proposal_id: id,
                original_base: commit.parents.first().cloned(),
                candidate,
                commit,
            });
        }
        let next = if more {
            entries.last().map(|entry| entry.proposal_id.clone())
        } else {
            None
        };
        Ok(ProposalPage { entries, next })
    }

    /// Inspect the actual current accepted/proposal versions and full diff; observed drift refuses instead of returning a stale tuple.
    pub fn proposal_diff(&self, proposal_id: &str, claim: &Claim) -> Result<ProposalDiff, Error> {
        validate_id(proposal_id)?;
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = self.scratch()?;
        let name = format!("refs/heads/omni-proposals/{proposal_id}");
        let expected_base = self.reference(ACCEPTED, scratch.path(), &call)?;
        let candidate = self
            .reference(&name, scratch.path(), &call)?
            .ok_or(Error::Invalid)?;
        let tree = self.base_tree(Some(&candidate), scratch.path(), &call)?;
        let comparison = self.compare_tree(expected_base.as_ref(), &tree, scratch.path(), &call)?;
        let commit = self.commit_info(&candidate, scratch.path(), &call)?;
        if self.reference(ACCEPTED, scratch.path(), &call)? != expected_base
            || self.reference(&name, scratch.path(), &call)? != Some(candidate.clone())
        {
            return Err(Error::Conflict);
        }
        Ok(ProposalDiff {
            proposal: Proposal {
                proposal_id: proposal_id.to_owned(),
                original_base: commit.parents.first().cloned(),
                candidate,
                commit,
            },
            expected_base,
            comparison,
        })
    }

    /// Read a bounded accepted first-parent page; a supplied OID is requalified against actual accepted ancestry.
    pub fn commits(
        &self,
        start: Option<&Oid>,
        limit: usize,
        claim: &Claim,
    ) -> Result<CommitPage, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        check_limit(limit, &call)?;
        let scratch = self.scratch()?;
        let accepted = self.reference(ACCEPTED, scratch.path(), &call)?;
        let Some(accepted) = accepted else {
            return Ok(CommitPage {
                entries: Vec::new(),
                next: None,
            });
        };
        let start = start.unwrap_or(&accepted);
        self.git(
            &[
                "merge-base",
                "--is-ancestor",
                start.as_str(),
                accepted.as_str(),
            ],
            b"",
            scratch.path(),
            None,
            &call,
        )
        .map_err(|error| {
            if error == Error::Git {
                Error::ContextMismatch
            } else {
                error
            }
        })?;
        let count = format!("--max-count={limit}");
        let listing = self.git(
            &["rev-list", "--first-parent", &count, start.as_str(), "--"],
            b"",
            scratch.path(),
            None,
            &call,
        )?;
        let mut entries = Vec::new();
        for oid in std::str::from_utf8(&listing)
            .map_err(|_| Error::Recovery)?
            .lines()
        {
            call.account_files(1)?;
            entries.push(self.commit_info(&Oid::parse(oid)?, scratch.path(), &call)?);
        }
        let next = entries
            .last()
            .and_then(|entry| entry.parents.first().cloned());
        Ok(CommitPage { entries, next })
    }

    /// Read actual metadata only for a commit reachable from admitted accepted/proposal history, never a naked hash grant.
    pub fn version(&self, version: &Oid, claim: &Claim) -> Result<CommitInfo, Error> {
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = self.scratch()?;
        self.qualify_version(version, scratch.path(), &call)?;
        self.commit_info(version, scratch.path(), &call)
    }

    /// Read selected text only after qualifying the immutable version against current accepted/proposal history.
    /// M1's unpublished-candidate installer keeps the older trusted read_text seam; public readers use this one.
    pub fn version_text(&self, version: &Oid, path: &str, claim: &Claim) -> Result<String, Error> {
        self.validate_path(path)?;
        let call = Call::new(claim)?;
        self.validate()?;
        let scratch = self.scratch()?;
        self.qualify_version(version, scratch.path(), &call)?;
        self.base_tree(Some(version), scratch.path(), &call)?;
        if self.binding.existing.is_some() {
            self.editable_base(version, path, scratch.path(), &call)?;
        }
        let spec = format!("{}:{path}", version.as_str());
        String::from_utf8(self.git(
            &["cat-file", "blob", &spec],
            b"",
            scratch.path(),
            None,
            &call,
        )?)
        .map_err(|_| Error::Recovery)
    }

    /// Read selected retained candidate bytes while a required signer is locked/missing; this creates no unsigned commit.
    pub fn read_candidate(
        &self,
        operation_id: &str,
        path: &str,
        claim: &Claim,
    ) -> Result<String, Error> {
        validate_id(operation_id)?;
        self.validate_path(path)?;
        let call = Call::new(claim)?;
        let _lock = self.lock()?;
        let intent = self.read_intent(operation_id)?;
        let scratch = self.scratch()?;
        if self.reference(
            &format!("refs/omni/trees/{operation_id}"),
            scratch.path(),
            &call,
        )? != Some(intent.prepared.tree.clone())
        {
            return Err(Error::Recovery);
        }
        if self.binding.existing.is_some() {
            self.editable_base(&intent.prepared.tree, path, scratch.path(), &call)?;
        }
        let spec = format!("{}:{path}", intent.prepared.tree.as_str());
        String::from_utf8(self.git(
            &["cat-file", "blob", &spec],
            b"",
            scratch.path(),
            None,
            &call,
        )?)
        .map_err(|_| Error::Recovery)
    }

    /// Qualify only accepted/proposal ancestry; prepared and cache hashes do not become public read authority.
    pub(crate) fn qualify_version(
        &self,
        version: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<(), Error> {
        let refs = self.git(
            &[
                "for-each-ref",
                "--format=%(refname) %(objectname) %(symref)",
                ACCEPTED,
                "refs/heads/omni-proposals/",
            ],
            b"",
            scratch,
            None,
            call,
        )?;
        for line in std::str::from_utf8(&refs)
            .map_err(|_| Error::Recovery)?
            .lines()
        {
            call.account_files(1)?;
            let fields: Vec<_> = line.split(' ').collect();
            if fields.len() != 3 || !fields[2].is_empty() {
                return Err(Error::Recovery);
            }
            if fields[0] != ACCEPTED {
                let id = fields[0]
                    .strip_prefix("refs/heads/omni-proposals/")
                    .ok_or(Error::Recovery)?;
                validate_id(id).map_err(|_| Error::Recovery)?;
            }
            let head = Oid::parse(fields[1])?;
            match self.git(
                &[
                    "merge-base",
                    "--is-ancestor",
                    version.as_str(),
                    head.as_str(),
                ],
                b"",
                scratch,
                None,
                call,
            ) {
                Ok(_) => return Ok(()),
                Err(Error::Git) => {}
                Err(error) => return Err(error),
            }
        }
        Err(Error::ContextMismatch)
    }

    /// Decode bounded native commit bytes without inventing author, committer, operation, or signature facts.
    pub(crate) fn commit_info(
        &self,
        oid: &Oid,
        scratch: &Path,
        call: &Call,
    ) -> Result<CommitInfo, Error> {
        let bytes = self.git(
            &["cat-file", "commit", oid.as_str()],
            b"",
            scratch,
            None,
            call,
        )?;
        let text = std::str::from_utf8(&bytes).map_err(|_| Error::Recovery)?;
        let (headers, message) = text.split_once("\n\n").ok_or(Error::Recovery)?;
        let mut tree = None;
        let mut parents = Vec::new();
        let mut author = None;
        let mut committer = None;
        let mut signature = SignatureFormat::Unsigned;
        for line in headers.lines() {
            if let Some(value) = line.strip_prefix("tree ") {
                tree = Some(Oid::parse(value)?);
            } else if let Some(value) = line.strip_prefix("parent ") {
                call.account_files(1)?;
                parents.push(Oid::parse(value)?);
            } else if let Some(value) = line.strip_prefix("author ") {
                author = Some(person(value)?);
            } else if let Some(value) = line.strip_prefix("committer ") {
                committer = Some(person(value)?);
            } else if let Some(value) = line.strip_prefix("gpgsig ") {
                signature = if value == "-----BEGIN SSH SIGNATURE-----" {
                    SignatureFormat::Ssh
                } else {
                    SignatureFormat::Other
                };
            }
        }
        let mut attribution = RecordedAttribution::default();
        let trailers = message
            .trim_end_matches('\n')
            .rsplit("\n\n")
            .next()
            .unwrap_or("");
        for line in trailers.lines() {
            let Some((name, value)) = line.split_once(": ") else {
                continue;
            };
            if value.is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
                continue;
            }
            let field = match name {
                "Omni-Operation-Id" => {
                    validate_id(value)?;
                    &mut attribution.operation_id
                }
                "Omni-Agent" => &mut attribution.agent,
                "Omni-Tool" => &mut attribution.tool,
                "Omni-Session-Id" => &mut attribution.session,
                "Omni-Origin" => &mut attribution.origin,
                "Omni-Observation-Id" => {
                    validate_id(value).map_err(|_| Error::Recovery)?;
                    attribution.observation_ids.push(value.to_owned());
                    continue;
                }
                _ => continue,
            };
            if field.is_some() {
                return Err(Error::Recovery);
            }
            *field = Some(value.to_owned());
        }
        let listing = self.git(
            &[
                "diff-tree",
                "--root",
                "--no-commit-id",
                "--first-parent",
                "-m",
                "-r",
                "--name-only",
                "-z",
                "--no-renames",
                oid.as_str(),
                "--",
                &self.binding.text_root,
                "sources",
                "omni-knowledge.json",
                "AGENTS.md",
                "schema.yaml",
            ],
            b"",
            scratch,
            None,
            call,
        )?;
        let mut affected_paths = Vec::new();
        for path in listing
            .split(|byte| *byte == 0)
            .filter(|part| !part.is_empty())
        {
            call.account_files(1)?;
            let path = std::str::from_utf8(path).map_err(|_| Error::Recovery)?;
            if self.validate_path(path).is_ok() {
                affected_paths.push(path.to_owned());
            }
        }
        affected_paths.sort();
        affected_paths.dedup();
        Ok(CommitInfo {
            version: oid.clone(),
            tree: tree.ok_or(Error::Recovery)?,
            parents,
            author: author.ok_or(Error::Recovery)?,
            committer: committer.ok_or(Error::Recovery)?,
            message: message.to_owned(),
            affected_paths,
            attribution,
            signature,
        })
    }
}

/// Enforce caller-selected finite page size before building any history list.
fn check_limit(limit: usize, call: &Call) -> Result<(), Error> {
    if limit == 0 || limit as u64 > call.files {
        Err(Error::Capacity)
    } else {
        Ok(())
    }
}

/// Preserve actual Git identity fields and date without treating them as OS authorization.
fn person(value: &str) -> Result<GitPerson, Error> {
    let (identity, clock) = value.rsplit_once("> ").ok_or(Error::Recovery)?;
    let (name, email) = identity.rsplit_once(" <").ok_or(Error::Recovery)?;
    let (seconds, timezone) = clock.split_once(' ').ok_or(Error::Recovery)?;
    if name.is_empty()
        || email.is_empty()
        || name.len() > 256
        || email.len() > 256
        || name.chars().chain(email.chars()).any(char::is_control)
        || timezone.len() != 5
        || !matches!(timezone.as_bytes()[0], b'+' | b'-')
        || !timezone.as_bytes()[1..].iter().all(u8::is_ascii_digit)
    {
        return Err(Error::Recovery);
    }
    Ok(GitPerson {
        name: name.to_owned(),
        email: email.to_owned(),
        seconds: seconds.parse().map_err(|_| Error::Recovery)?,
        timezone: timezone.to_owned(),
    })
}
