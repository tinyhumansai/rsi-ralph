//! Durable provider and checker callbacks for one controlled host invocation.
use crate::{
    Error, EvidenceJournal, ModelFuture, ModelPort, ModelRequest, Result, VerificationEvidence,
    VerificationObserver,
};
use std::sync::Arc;
use tinyhivemind_hives::TurnRequest;

#[derive(Clone)]
pub(super) struct Recording {
    journal: Arc<EvidenceJournal>,
    id: u64,
}
impl Recording {
    /// Persists a bound invocation intent before the controlled provider dispatch.
    pub(super) fn begin(journal: Arc<EvidenceJournal>, request: &TurnRequest) -> Result<Self> {
        let episode = request.episode.as_ref().ok_or(Error::InvalidEvidence)?;
        let id = journal.begin_turn(&request.agent_id, &episode.episode_id)?;
        Ok(Self { journal, id })
    }
    /// Wraps provider accounting while leaving returned artifact bytes private.
    pub(super) fn model(&self, inner: Arc<dyn ModelPort>) -> Arc<dyn ModelPort> {
        Arc::new(RecordedModel {
            inner,
            recording: self.clone(),
        })
    }
    /// Closes the invocation before the runner outcome reaches the Coordinator.
    pub(super) fn finish(&self) -> Result<()> {
        self.journal.finish_turn(self.id)
    }
}
impl VerificationObserver for Recording {
    /// Persists actual checker evidence before verified completion is staged.
    fn observe(&self, evidence: &VerificationEvidence) -> Result<()> {
        self.journal
            .record_verification(self.id, evidence.clone())
            .map(|_| ())
    }
}
struct RecordedModel {
    inner: Arc<dyn ModelPort>,
    recording: Recording,
}
impl ModelPort for RecordedModel {
    /// Keeps provider identity intact through accounting wrappers.
    fn identity(&self) -> Option<crate::ModelIdentity> {
        self.inner.identity()
    }
    /// Saves measured tokens before later response-cap or checker rejection.
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let future = self.inner.generate(request);
        let recording = self.recording.clone();
        Box::pin(async move {
            let reply = future.await?;
            // Persist even a response that later fails request cap validation.
            // The returned content remains private and never enters the journal.
            // A journal failure ends the call: the port boundary only carries
            // model errors, and unrecorded usage must not reach the caller.
            recording
                .journal
                .record_usage(recording.id, reply.input_tokens, reply.output_tokens)
                .map_err(|_| crate::ModelError::CallFailed)?;
            Ok(reply)
        })
    }
}
