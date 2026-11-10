//! Transport-neutral Protocol V2 client request namespace and uncertainty fence.

use agora_protocol::v2::{ClientIdV2, CommandV2, ReplyV2, RequestIdV2, RequestV2};

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
}
