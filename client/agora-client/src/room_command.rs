//! Exact-confirmed room creation over the ordinary Protocol V2 client.

use crate::v2::{ClientErrorV2, ClientV2, ServicePortV2};
use agora_protocol::v2::{AdmissionPolicyV2, CommandV2, OutcomeV2, RefusalV2, ReplyV2};

pub const CREATE_CAPABILITY: &str = "rooms.create.v2";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRoomRequestV1 {
    pub room_id: String,
    pub name: String,
    pub admission: AdmissionPolicyV2,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRoomReceiptV1 {
    pub room_id: String,
    pub revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreateRoomErrorV1<E> {
    ConfirmationRefused { required: String },
    Client(ClientErrorV2<E>),
    Refused(RefusalV2),
    UnexpectedReply,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreateRoomConfirmationRefusalV1 {
    pub required: String,
}

pub fn required_create_confirmation(request: &CreateRoomRequestV1) -> String {
    let admission = match request.admission {
        AdmissionPolicyV2::Open => "OPEN",
        AdmissionPolicyV2::InviteOnly => "INVITE ONLY",
    };
    format!(
        "CREATE AGORA ROOM {} NAMED {} ADMISSION {} AT REVISION {}",
        request.room_id, request.name, admission, request.expected_revision
    )
}

pub fn confirm_create_room(
    request: &CreateRoomRequestV1,
    confirmation: &str,
) -> Result<(), CreateRoomConfirmationRefusalV1> {
    let required = required_create_confirmation(request);
    if confirmation == required {
        Ok(())
    } else {
        Err(CreateRoomConfirmationRefusalV1 { required })
    }
}

/// Attempts the exact revision-bound command once. If delivery is uncertain,
/// the caller must retain this client and submit the identical request again;
/// `ClientV2` reuses the retained request identity and fences every other
/// command until that outcome resolves.
pub fn create_room<P: ServicePortV2>(
    client: &mut ClientV2<P>,
    request: CreateRoomRequestV1,
    confirmation: &str,
) -> Result<CreateRoomReceiptV1, CreateRoomErrorV1<P::Error>> {
    confirm_create_room(&request, confirmation).map_err(|refusal| {
        CreateRoomErrorV1::ConfirmationRefused {
            required: refusal.required,
        }
    })?;
    let room_id = request.room_id.clone();
    let reply = client
        .execute(CommandV2::CreateRoom {
            room_id: request.room_id,
            name: request.name,
            admission: request.admission,
            expected_revision: request.expected_revision,
        })
        .map_err(CreateRoomErrorV1::Client)?;
    match reply {
        ReplyV2::Accepted(OutcomeV2::RoomCreated { revision }) => {
            Ok(CreateRoomReceiptV1 { room_id, revision })
        }
        ReplyV2::Refused(refusal) => Err(CreateRoomErrorV1::Refused(refusal)),
        ReplyV2::Accepted(_) => Err(CreateRoomErrorV1::UnexpectedReply),
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

    fn request() -> CreateRoomRequestV1 {
        CreateRoomRequestV1 {
            room_id: "delivery".into(),
            name: "Delivery Room".into(),
            admission: AdmissionPolicyV2::InviteOnly,
            expected_revision: 7,
        }
    }

    #[test]
    fn refuses_before_transport_without_the_exact_confirmation() {
        let mut client = ClientV2::new(ClientIdV2(4), Port::default());
        let error = create_room(&mut client, request(), "CREATE SOMETHING").unwrap_err();
        assert_eq!(
            error,
            CreateRoomErrorV1::ConfirmationRefused {
                required: required_create_confirmation(&request()),
            }
        );
        let (port, pending) = client.into_parts();
        assert!(port.requests.is_empty());
        assert!(pending.is_none());
    }

    #[test]
    fn returns_the_revision_bound_owner_receipt() {
        let port = Port {
            replies: [Ok(ReplyV2::Accepted(OutcomeV2::RoomCreated {
                revision: 8,
            }))]
            .into(),
            ..Default::default()
        };
        let mut client = ClientV2::new(ClientIdV2(4), port);
        let selected = request();
        let receipt = create_room(
            &mut client,
            selected.clone(),
            &required_create_confirmation(&selected),
        )
        .unwrap();
        assert_eq!(
            receipt,
            CreateRoomReceiptV1 {
                room_id: "delivery".into(),
                revision: 8,
            }
        );
    }

    #[test]
    fn exact_retry_reuses_the_request_identity_after_outcome_unknown() {
        let port = Port {
            replies: [
                Err(PortFailureV2::OutcomeUnknown("lost")),
                Ok(ReplyV2::Accepted(OutcomeV2::RoomCreated { revision: 8 })),
            ]
            .into(),
            ..Default::default()
        };
        let mut client = ClientV2::new(ClientIdV2(4), port);
        let selected = request();
        let confirmation = required_create_confirmation(&selected);
        assert_eq!(
            create_room(&mut client, selected.clone(), &confirmation),
            Err(CreateRoomErrorV1::Client(ClientErrorV2::OutcomeUnknown(
                "lost"
            )))
        );
        let receipt = create_room(&mut client, selected, &confirmation).unwrap();
        assert_eq!(receipt.revision, 8);
        let (port, pending) = client.into_parts();
        assert!(pending.is_none());
        assert_eq!(port.requests.len(), 2);
        assert_eq!(port.requests[0], port.requests[1]);
    }
}
