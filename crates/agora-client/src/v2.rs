//! Transport-neutral Protocol V2 client request namespace and uncertainty fence.

use agora_protocol::v2::{ClientIdV2, CommandV2, ReplyV2, RequestIdV2, RequestV2};

#[derive(Clone, Eq, PartialEq)]
pub struct ClientStateV2 {
    pub client: ClientIdV2,
    pub next_request: u64,
    pub pending: Option<RequestV2>,
}

impl std::fmt::Debug for ClientStateV2 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ClientStateV2")
            .field("client", &self.client)
            .field("next_request", &self.next_request)
            .field(
                "pending_request",
                &self.pending.as_ref().map(|request| request.request_id),
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClientRestoreErrorV2 {
    InvalidPendingRequest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PortFailureV2<E> {
    NotSent(E),
    OutcomeUnknown(E),
}

pub trait ServicePortV2 {
    type Error;
    fn execute(
        &mut self,
        client: ClientIdV2,
        request: RequestV2,
    ) -> Result<ReplyV2, PortFailureV2<Self::Error>>;
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClientErrorV2<E> {
    RequestIdExhausted,
    NotSent(E),
    OutcomeUnknown(E),
    UnresolvedOutcome,
}

#[derive(Debug)]
pub struct ClientV2<P> {
    client: ClientIdV2,
    next_request: u64,
    pending: Option<RequestV2>,
    port: P,
}

impl<P: ServicePortV2> ClientV2<P> {
    pub fn new(client: ClientIdV2, port: P) -> Self {
        Self {
            client,
            next_request: 0,
            pending: None,
            port,
        }
    }

    /// Restores only the client-owned request namespace. Durable storage and
    /// endpoint/session correspondence remain caller-owned deployment policy.
    pub fn restore(state: ClientStateV2, port: P) -> Result<Self, ClientRestoreErrorV2> {
        if let Some(pending) = &state.pending
            && state.next_request.checked_sub(1) != Some(pending.request_id.0)
        {
            return Err(ClientRestoreErrorV2::InvalidPendingRequest);
        }
        Ok(Self {
            client: state.client,
            next_request: state.next_request,
            pending: state.pending,
            port,
        })
    }

    pub fn execute(&mut self, command: CommandV2) -> Result<ReplyV2, ClientErrorV2<P::Error>> {
        if let Some(pending) = &self.pending {
            if pending.command != command {
                return Err(ClientErrorV2::UnresolvedOutcome);
            }
            let request = pending.clone();
            return match self.port.execute(self.client, request) {
                Ok(reply) => {
                    self.pending = None;
                    Ok(reply)
                }
                Err(PortFailureV2::NotSent(error)) => Err(ClientErrorV2::NotSent(error)),
                Err(PortFailureV2::OutcomeUnknown(error)) => {
                    Err(ClientErrorV2::OutcomeUnknown(error))
                }
            };
        }
        let request_id = RequestIdV2(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(ClientErrorV2::RequestIdExhausted)?;
        let request = RequestV2 {
            request_id,
            command,
        };
        match self.port.execute(self.client, request.clone()) {
            Ok(reply) => Ok(reply),
            Err(PortFailureV2::NotSent(error)) => Err(ClientErrorV2::NotSent(error)),
            Err(PortFailureV2::OutcomeUnknown(error)) => {
                self.pending = Some(request);
                Err(ClientErrorV2::OutcomeUnknown(error))
            }
        }
    }

    pub fn pending_request(&self) -> Option<&RequestV2> {
        self.pending.as_ref()
    }

    pub fn state(&self) -> ClientStateV2 {
        ClientStateV2 {
            client: self.client,
            next_request: self.next_request,
            pending: self.pending.clone(),
        }
    }

    /// Explicitly abandons client-side uncertainty. This does not cancel or
    /// disprove a service-side result, so the exact request is returned for
    /// durable or operator accounting.
    pub fn abandon_uncertainty(&mut self) -> Option<RequestV2> {
        self.pending.take()
    }

    /// Extracts both the port and any unresolved exact request.
    pub fn into_parts(self) -> (P, Option<RequestV2>) {
        (self.port, self.pending)
    }

    pub fn into_state(self) -> (P, ClientStateV2) {
        let state = ClientStateV2 {
            client: self.client,
            next_request: self.next_request,
            pending: self.pending,
        };
        (self.port, state)
    }
}
