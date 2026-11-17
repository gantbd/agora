//! Exact-confirmed message delivery over the ordinary Protocol V2 client.

use crate::v2::{ClientErrorV2, ClientV2, ServicePortV2};
use agora_protocol::v2::{CommandV2, OutcomeV2, RefusalV2, ReplyV2};

pub const SPEAK_CAPABILITY: &str = "messages.speak.v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendMessageRequestV1 {
    pub room_id: String,
    pub expected_revision: u64,
    pub logical_time: u64,
    pub body: String,
    pub addressed_to: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SendMessageReceiptV1 {
    pub room_id: String,
    pub revision: u64,
    pub sequence: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SendMessageErrorV1<E> {
    ConfirmationRefused { required: String },
    Client(ClientErrorV2<E>),
    Refused(RefusalV2),
    UnexpectedReply,
}

pub fn required_send_confirmation(request: &SendMessageRequestV1) -> String {
    format!(
        "SEND AGORA MESSAGE TO {} AT REVISION {} LOGICAL TIME {} BODY BYTES {} ADDRESSEES {}",
        request.room_id,
        request.expected_revision,
        request.logical_time,
        request.body.len(),
        request.addressed_to.len()
    )
}

/// Attempts the exact revision-bound command once. The caller must retain both
/// this client and the identical request when delivery is uncertain. `ClientV2`
/// then reuses the retained request identity and fences every different command.
pub fn send_message<P: ServicePortV2>(
    client: &mut ClientV2<P>,
    request: SendMessageRequestV1,
    confirmation: &str,
) -> Result<SendMessageReceiptV1, SendMessageErrorV1<P::Error>> {
    let required = required_send_confirmation(&request);
    if confirmation != required {
        return Err(SendMessageErrorV1::ConfirmationRefused { required });
    }
    let room_id = request.room_id.clone();
    let reply = client
        .execute(CommandV2::Speak {
            room_id: request.room_id,
            expected_revision: request.expected_revision,
            logical_time: request.logical_time,
            body: request.body,
            addressed_to: request.addressed_to,
        })
        .map_err(SendMessageErrorV1::Client)?;
    match reply {
        ReplyV2::Accepted(OutcomeV2::MessageAccepted { revision, sequence }) => {
            Ok(SendMessageReceiptV1 {
                room_id,
                revision,
                sequence,
            })
        }
        ReplyV2::Refused(refusal) => Err(SendMessageErrorV1::Refused(refusal)),
        ReplyV2::Accepted(_) => Err(SendMessageErrorV1::UnexpectedReply),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::PortFailureV2;
    use agora_protocol::v2::{ClientIdV2, RequestV2};
    use std::collections::VecDeque;

    #[derive(Default)]
    struct Port {
        requests: Vec<RequestV2>,
        replies: VecDeque<Result<ReplyV2, PortFailureV2<&'static str>>>,
    }

    impl ServicePortV2 for Port {
        type Error = &'static str;

        fn execute(
            &mut self,
            _: ClientIdV2,
            request: RequestV2,
        ) -> Result<ReplyV2, PortFailureV2<Self::Error>> {
            self.requests.push(request);
            self.replies
                .pop_front()
                .unwrap_or(Ok(ReplyV2::Refused(RefusalV2::Unavailable)))
        }
    }

    fn request() -> SendMessageRequestV1 {
        SendMessageRequestV1 {
            room_id: "delivery".into(),
            expected_revision: 7,
            logical_time: 41,
            body: "first line\nsecond line".into(),
            addressed_to: vec!["ada".into(), "grace".into()],
        }
    }

    #[test]
    fn refuses_before_transport_without_exact_confirmation() {
        let mut client = ClientV2::new(ClientIdV2(4), Port::default());
        assert!(matches!(
            send_message(&mut client, request(), "SEND SOMETHING"),
            Err(SendMessageErrorV1::ConfirmationRefused { .. })
        ));
        let (port, pending) = client.into_parts();
        assert!(port.requests.is_empty());
        assert!(pending.is_none());
    }

    #[test]
    fn returns_revision_and_sequence_owner_receipt() {
        let port = Port {
            replies: [Ok(ReplyV2::Accepted(OutcomeV2::MessageAccepted {
                revision: 8,
                sequence: 12,
            }))]
            .into(),
            ..Default::default()
        };
        let mut client = ClientV2::new(ClientIdV2(4), port);
        let selected = request();
        let receipt = send_message(
            &mut client,
            selected.clone(),
            &required_send_confirmation(&selected),
        )
        .unwrap();
        assert_eq!(
            receipt,
            SendMessageReceiptV1 {
                room_id: "delivery".into(),
                revision: 8,
                sequence: 12,
            }
        );
    }

    #[test]
    fn exact_retry_reuses_request_identity_after_outcome_unknown() {
        let port = Port {
            replies: [
                Err(PortFailureV2::OutcomeUnknown("lost")),
                Ok(ReplyV2::Accepted(OutcomeV2::MessageAccepted {
                    revision: 8,
                    sequence: 12,
                })),
            ]
            .into(),
            ..Default::default()
        };
        let mut client = ClientV2::new(ClientIdV2(4), port);
        let selected = request();
        let confirmation = required_send_confirmation(&selected);
        assert_eq!(
            send_message(&mut client, selected.clone(), &confirmation),
            Err(SendMessageErrorV1::Client(ClientErrorV2::OutcomeUnknown(
                "lost"
            )))
        );
        let receipt = send_message(&mut client, selected, &confirmation).unwrap();
        assert_eq!(receipt.sequence, 12);
        let (port, pending) = client.into_parts();
        assert!(pending.is_none());
        assert_eq!(port.requests.len(), 2);
        assert_eq!(port.requests[0], port.requests[1]);
    }
}
