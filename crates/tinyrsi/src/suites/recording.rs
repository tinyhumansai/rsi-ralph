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
    pub(super) fn begin(journal: Arc<EvidenceJournal>, request: &TurnRequest) -> Result<Self> {
        let episode = request.episode.as_ref().ok_or(Error::InvalidEvidence)?;
        let id = journal.begin_turn(&request.agent_id, &episode.episode_id)?;
        Ok(Self { journal, id })
    }
    pub(super) fn model(&self, inner: Arc<dyn ModelPort>) -> Arc<dyn ModelPort> {
        Arc::new(RecordedModel {
            inner,
            recording: self.clone(),
        })
    }
    pub(super) fn finish(&self) -> Result<()> {
        self.journal.finish_turn(self.id)
    }
}
impl VerificationObserver for Recording {
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
    fn generate(&self, request: ModelRequest) -> ModelFuture {
        let future = self.inner.generate(request);
        let recording = self.recording.clone();
        Box::pin(async move {
            let reply = future.await?;
            // Persist even a response that later fails request cap validation.
            // The returned content remains private and never enters the journal.
            recording.journal.record_usage(
                recording.id,
                reply.input_tokens,
                reply.output_tokens,
            )?;
            Ok(reply)
        })
    }
}
