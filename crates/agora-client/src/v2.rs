//! Transport-neutral Protocol V2 client request namespace.

use agora_protocol::v2::{ClientIdV2, CommandV2, ReplyV2, RequestIdV2, RequestV2};

pub trait ServicePortV2 {
    type Error;
    fn execute(&mut self, client: ClientIdV2, request: RequestV2) -> Result<ReplyV2, Self::Error>;
}

#[derive(Debug)]
pub enum ClientErrorV2<E> {
    RequestIdExhausted,
    Access(E),
}

#[derive(Debug)]
pub struct ClientV2<P> {
    client: ClientIdV2,
    next_request: u64,
    port: P,
}

impl<P: ServicePortV2> ClientV2<P> {
    pub fn new(client: ClientIdV2, port: P) -> Self {
        Self {
            client,
            next_request: 0,
            port,
        }
    }

    pub fn execute(&mut self, command: CommandV2) -> Result<ReplyV2, ClientErrorV2<P::Error>> {
        let request_id = RequestIdV2(self.next_request);
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(ClientErrorV2::RequestIdExhausted)?;
        self.port
            .execute(
                self.client,
                RequestV2 {
                    request_id,
                    command,
                },
            )
            .map_err(ClientErrorV2::Access)
    }

    pub fn into_port(self) -> P {
        self.port
    }
}
