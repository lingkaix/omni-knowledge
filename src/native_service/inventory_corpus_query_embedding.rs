//! Actual Query/model/Claim text embedding over the ONE existing funded provider.
//! This is not source-vector eligibility, all-method delivery, loaded weights or quality acceptance.
use super::*;
use knowledge_processing::embedding::funded::{
    EmbeddedText, EmbeddingControl, FundedTextProvider, ProviderRefusal,
};

/// Actual selected model/package/receiver, SDK invocation, Claim and affine provider output.
/// Failed work stays in this foreground owner; explicit `finish` follows the last vector consumer.
pub struct NativeQueryEmbedding {
    failure: Option<anyhow::Error>,
    materialized: Option<Vec<f32>>,
    output: Option<EmbeddedText>,
    provider: Option<FundedTextProvider>,
    credits: Vec<Holding>,
    vector_credit: Option<Holding>,
    query: NativeTupleQuery,
    model: Option<NativeEmbeddingSelection>,
    claim: Arc<Claim>,
    library: usize,
    closing: bool,
}
impl NativeTupleQuery {
    /// Run only a semantic keyword selected by this SAME actual Query and native model owner.
    /// The enclosing caller must prepay the complete provider plus its other simultaneous peaks.
    /// Always return the actual owner, including construction/request/budget/dependency failures.
    pub fn embed_keyword(
        &self,
        claim: Arc<Claim>,
        library: usize,
        keyword: usize,
    ) -> NativeQueryEmbedding {
        let mut owner = NativeQueryEmbedding {
            output: None,
            materialized: None,
            failure: None,
            provider: None,
            credits: Vec::new(),
            vector_credit: None,
            query: NativeTupleQuery {
                cause: self.cause.clone(),
            },
            model: None,
            claim,
            library,
            closing: false,
        };
        if let Err(error) = owner.begin(keyword) {
            owner.failure = Some(error);
        }
        owner
    }
}
impl NativeQueryEmbedding {
    fn begin(&mut self, keyword: usize) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        let library = self
            .query
            .cause
            .libraries
            .get(self.library)
            .context("actual Query library")?;
        let wire::Command::Query { query } = library
            .reader
            .permit
            .admitted_command()
            .map_err(refused_error)?
        else {
            bail!("actual Query required")
        };
        anyhow::ensure!(
            query.methods.contains(&wire::SearchMethod::Semantic),
            "actual selected semantic method required"
        );
        anyhow::ensure!(
            query.image_query.is_none(),
            "actual text keyword required; image receiver is a separate dependency"
        );
        let text = query
            .keywords
            .get(keyword)
            .context("actual Query keyword required")?
            .clone();
        let model = library
            .model
            .as_ref()
            .context("actual selected semantic model owner required")?
            .clone();
        model.check_permit(&library.reader.permit)?;
        anyhow::ensure!(
            library.reader.physical.data.model.as_ref() == Some(model.model()?),
            "actual tuple semantic dependency differs"
        );
        let vector_bytes = (model.model()?.dimension as u64)
            .checked_mul(4)
            .context("query vector size overflow")?;
        // Known keyword/output DATA is admitted before construction/contact. Generic provider
        // HTTP metadata/response ceilings remain its existing fixed transport contract; this
        // primitive is not qualification of a complete all-method transport-data allowance.
        self.query.cause.admit_io(
            &self.claim,
            (text.len() as u64)
                .checked_add(vector_bytes)
                .context("query embedding data overflow")?,
        )?;
        let config = model.config()?.clone();
        let need = FundedTextProvider::requirement(&config)?;
        let paid = Account::current()?.split_claim(&self.claim, self.query.key(), need)?;
        self.model = Some(model);
        let provider = FundedTextProvider::open_prepaid(&config, vec![paid])?;
        self.output = Some(provider.embed_text(&text, self)?);
        self.checkpoint()
    }
    /// Actual retained failure, never an empty successful semantic result or settled-work marker.
    pub fn failure(&self) -> Option<&anyhow::Error> {
        self.failure.as_ref()
    }
    /// Borrow only while the actual model/SDK/Claim remains current. Copying requires the next
    /// consumer's separately prepaid buffer before this output/provider can be destroyed.
    pub fn vector(&self) -> Result<&[f32]> {
        anyhow::ensure!(
            !self.closing && self.failure.is_none(),
            "native embedding is not deliverable"
        );
        self.checkpoint()?;
        if let Some(vector) = &self.materialized {
            return Ok(vector);
        }
        self.output
            .as_ref()
            .context("actual affine embedding output required")?
            .vector(self)
    }
    pub(super) fn vector_for(
        &self,
        cause: &Arc<QueryCause>,
        claim: &Arc<Claim>,
        library: usize,
    ) -> Result<&[f32]> {
        anyhow::ensure!(
            Arc::ptr_eq(cause, &self.query.cause)
                && Arc::ptr_eq(claim, &self.claim)
                && library == self.library,
            "foreign actual Query/model vector owner"
        );
        self.vector()
    }
    /// Prepay the next real vector buffer from THIS same Claim before copying, then destroy
    /// the original output and synchronously settle its actual provider. The small buffer keeps
    /// its own actual Holding; returning provider credit is NOT release of the live copied data.
    /// Failure leaves all real buffers/provider/loans in this same foreground owner.
    pub fn materialize(&mut self) -> Result<()> {
        let bytes = (self.vector()?.len() as u64)
            .checked_mul(4)
            .context("query vector size overflow")?;
        if self.materialized.is_none() {
            if self.vector_credit.is_none() {
                let need = Reservation {
                    memory_bytes: bytes
                        .checked_mul(2)
                        .and_then(|n| n.checked_add(4096))
                        .context("query vector buffer quote overflow")?,
                    temporary_bytes: 0,
                };
                self.vector_credit =
                    Some(Account::current()?.split_claim(&self.claim, self.query.key(), need)?);
            }
            self.materialized = Some(self.vector()?.to_vec());
            if let Some(output) = self.output.take() {
                self.provider = Some(output.into_provider());
            }
        }
        self.settle_provider()?;
        self.rejoin_credits()?;
        self.checkpoint()
    }
    fn settle_provider(&mut self) -> Result<()> {
        if let Some(provider) = self.provider.take() {
            match provider.into_holdings() {
                Ok(credits) => self.credits.extend(credits),
                Err(provider) => {
                    self.provider = Some(*provider);
                    bail!("actual native query provider remains foreground-owned")
                }
            }
        }
        Ok(())
    }
    fn rejoin_credits(&mut self) -> Result<()> {
        while let Some(holding) = self.credits.last_mut() {
            Account::current()?.rejoin_claim(holding, &self.claim, self.query.key())?;
            self.credits.pop();
        }
        Ok(())
    }
    /// Destroy vector/error data, actual transport/runtime/tasks/IO, then rejoin the ACTUAL loans.
    /// Ambient runtime/live cleanup failures return THIS actual owner; no replacement reserve.
    pub fn finish(mut self) -> std::result::Result<(NativeTupleQuery, Arc<Claim>), Box<Self>> {
        self.closing = true;
        if let Some(error) = self.failure.as_mut()
            && let Some(owner) = error.downcast_mut::<ProviderRefusal>()
            && let Some(provider) = owner.take_provider()
        {
            self.provider = Some(provider);
        }
        drop(self.failure.take());
        drop(self.materialized.take());
        if let Some(output) = self.output.take() {
            self.provider = Some(output.into_provider());
        }
        if let Err(error) = self.settle_provider() {
            self.failure = Some(error);
            return Err(Box::new(self));
        }
        if let Some(credit) = self.vector_credit.take() {
            self.credits.push(credit);
        }
        if let Err(error) = self.rejoin_credits() {
            self.failure = Some(error);
            return Err(Box::new(self));
        }
        Ok((self.query, self.claim))
    }
}
impl EmbeddingControl for NativeQueryEmbedding {
    fn checkpoint(&self) -> Result<()> {
        self.query.cause.check_claim(&self.claim)?;
        self.model
            .as_ref()
            .context("actual model owner required")?
            .check_permit(&self.query.cause.libraries[self.library].reader.permit)
    }
    fn deadline(&self) -> Instant {
        self.claim.deadline().min(self.query.cause.deadline)
    }
}
