//! Session-and-server-bound client uncertainty for the volatile collaboration profile.

use agora_protocol::transient::{
    TransientCommandV1, TransientReplyV1, TransientRequestIdV1, TransientRequestV1,
};
use std::fmt;

#[derive(Clone, Copy, Eq, Ord, PartialEq, PartialOrd)]
pub struct TransientReplayEpochV1([u8; 32]);

impl TransientReplayEpochV1 {
    /// The caller must change this value for every new authenticated Host
    /// session and every new server incarnation.
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        if bytes == [0; 32] {
            None
        } else {
            Some(Self(bytes))
        }
    }
}

impl fmt::Debug for TransientReplayEpochV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("TransientReplayEpochV1(<opaque>)")
    }
}

#[derive(Clone, Eq, PartialEq)]
pub struct TransientClientStateV1 {
    pub replay_epoch: TransientReplayEpochV1,
    pub next_request: u64,
    pub pending: Option<TransientRequestV1>,
}

impl fmt::Debug for TransientClientStateV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransientClientStateV1")
            .field("replay_epoch", &self.replay_epoch)
            .field("next_request", &self.next_request)
            .field(
                "pending_request",
                &self.pending.as_ref().map(|request| request.request_id),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransientClientRestoreErrorV1 {
    ReplayEpochChanged,
    InvalidPendingRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransientPortFailureV1<E> {
    NotSent(E),
    OutcomeUnknown(E),
}

pub trait TransientPortV1 {
    type Error;

    fn execute(
        &mut self,
        request: TransientRequestV1,
    ) -> Result<TransientReplyV1, TransientPortFailureV1<Self::Error>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransientClientErrorV1<E> {
    RequestIdExhausted,
    NotSent(E),
    OutcomeUnknown(E),
    UnresolvedOutcome,
}

#[derive(Debug)]
pub struct TransientClientV1<P> {
    replay_epoch: TransientReplayEpochV1,
    next_request: u64,
    pending: Option<TransientRequestV1>,
    port: P,
}

impl<P: TransientPortV1> TransientClientV1<P> {
    pub fn new(replay_epoch: TransientReplayEpochV1, port: P) -> Self {
        Self {
            replay_epoch,
            next_request: 0,
            pending: None,
            port,
        }
    }

    pub fn restore(
        current_replay_epoch: TransientReplayEpochV1,
        state: TransientClientStateV1,
        port: P,
    ) -> Result<Self, TransientClientRestoreErrorV1> {
        if current_replay_epoch != state.replay_epoch {
            return Err(TransientClientRestoreErrorV1::ReplayEpochChanged);
        }
        if let Some(pending) = &state.pending
            && state.next_request.checked_sub(1) != Some(pending.request_id.0)
        {
            return Err(TransientClientRestoreErrorV1::InvalidPendingRequest);
        }
        Ok(Self {
            replay_epoch: state.replay_epoch,
            next_request: state.next_request,
            pending: state.pending,
            port,
        })
    }

    pub fn execute(
        &mut self,
        command: TransientCommandV1,
    ) -> Result<TransientReplyV1, TransientClientErrorV1<P::Error>> {
        if let Some(pending) = &self.pending {
            if pending.command != command {
                return Err(TransientClientErrorV1::UnresolvedOutcome);
            }
            let request = pending.clone();
            return match self.port.execute(request) {
                Ok(reply) => {
                    self.pending = None;
                    Ok(reply)
                }
                Err(TransientPortFailureV1::NotSent(error)) => {
                    Err(TransientClientErrorV1::NotSent(error))
                }
                Err(TransientPortFailureV1::OutcomeUnknown(error)) => {
                    Err(TransientClientErrorV1::OutcomeUnknown(error))
                }
            };
        }
        let request_id = TransientRequestIdV1(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(TransientClientErrorV1::RequestIdExhausted)?;
        let request = TransientRequestV1 {
            request_id,
            command,
        };
        match self.port.execute(request.clone()) {
            Ok(reply) => Ok(reply),
            Err(TransientPortFailureV1::NotSent(error)) => {
                Err(TransientClientErrorV1::NotSent(error))
            }
            Err(TransientPortFailureV1::OutcomeUnknown(error)) => {
                self.pending = Some(request);
                Err(TransientClientErrorV1::OutcomeUnknown(error))
            }
        }
    }

    pub fn pending_request(&self) -> Option<&TransientRequestV1> {
        self.pending.as_ref()
    }

    pub fn state(&self) -> TransientClientStateV1 {
        TransientClientStateV1 {
            replay_epoch: self.replay_epoch,
            next_request: self.next_request,
            pending: self.pending.clone(),
        }
    }

    pub fn into_state(self) -> (P, TransientClientStateV1) {
        let state = TransientClientStateV1 {
            replay_epoch: self.replay_epoch,
            next_request: self.next_request,
            pending: self.pending,
        };
        (self.port, state)
    }

    /// Starts a fresh volatile namespace and returns any uncertainty from the
    /// prior server epoch for explicit operator or application accounting.
    pub fn rebind(
        self,
        replay_epoch: TransientReplayEpochV1,
    ) -> (Self, Option<TransientRequestV1>) {
        let unresolved = self.pending;
        (
            Self {
                replay_epoch,
                next_request: 0,
                pending: None,
                port: self.port,
            },
            unresolved,
        )
    }
}
