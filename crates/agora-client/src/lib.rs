//! Transport-neutral Agora client with a monotone request namespace.

use agora_protocol::{ClientIdV1, CommandV1, ReplyV1, RequestIdV1, RequestV1};

pub mod search;
pub mod transient;
pub mod v2;

pub trait ServicePort {
    type Error;
    fn execute(&mut self, client: ClientIdV1, request: RequestV1) -> Result<ReplyV1, Self::Error>;
}

#[derive(Debug)]
pub enum ClientError<E> {
    RequestIdExhausted,
    Access(E),
}

#[derive(Debug)]
pub struct Client<P> {
    client: ClientIdV1,
    next_request: u64,
    port: P,
}

impl<P: ServicePort> Client<P> {
    pub fn new(client: ClientIdV1, port: P) -> Self {
        Self {
            client,
            next_request: 0,
            port,
        }
    }

    pub fn execute(&mut self, command: CommandV1) -> Result<ReplyV1, ClientError<P::Error>> {
        let request_id = RequestIdV1(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(ClientError::RequestIdExhausted)?;
        self.port
            .execute(
                self.client,
                RequestV1 {
                    request_id,
                    command,
                },
            )
            .map_err(ClientError::Access)
    }

    pub fn into_port(self) -> P {
        self.port
    }
}
