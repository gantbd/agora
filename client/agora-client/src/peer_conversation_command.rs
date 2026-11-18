//! Exact-confirmed direct/group conversation creation over Protocol V2.

use crate::v2::{ClientErrorV2, ClientV2, ServicePortV2};
use agora_protocol::v2::{CommandV2, ConversationProfileV1, OutcomeV2, RefusalV2, ReplyV2};

pub const CREATE_PEER_CONVERSATION_CAPABILITY: &str = "conversations.peer.create.v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePeerConversationRequestV1 {
    pub room_id: String,
    pub name: String,
    pub profile: ConversationProfileV1,
    pub founding_participant_ids: Vec<String>,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CreatePeerConversationReceiptV1 {
    pub room_id: String,
    pub revision: u64,
    pub profile: ConversationProfileV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CreatePeerConversationErrorV1<E> {
    ConfirmationRefused { required: String },
    Client(ClientErrorV2<E>),
    Refused(RefusalV2),
    UnexpectedReply,
}

pub fn required_peer_conversation_confirmation(
    request: &CreatePeerConversationRequestV1,
) -> String {
    let mut founding_participant_ids = request.founding_participant_ids.clone();
    founding_participant_ids.sort();
    let profile = match request.profile {
        ConversationProfileV1::Direct => "DIRECT",
        ConversationProfileV1::GroupDirect => "GROUP DIRECT",
        ConversationProfileV1::Ordinary => "ORDINARY",
    };
    format!(
        "CREATE AGORA {} CONVERSATION {} NAMED {} WITH {} AT REVISION {}",
        profile,
        request.room_id,
        request.name,
        founding_participant_ids.join(","),
        request.expected_revision
    )
}

/// Attempts the exact revision-bound command once. The ordinary `ClientV2`
/// retained-request rules fence every different command after uncertainty.
pub fn create_peer_conversation<P: ServicePortV2>(
    client: &mut ClientV2<P>,
    mut request: CreatePeerConversationRequestV1,
    confirmation: &str,
) -> Result<CreatePeerConversationReceiptV1, CreatePeerConversationErrorV1<P::Error>> {
    let required = required_peer_conversation_confirmation(&request);
    if confirmation != required {
        return Err(CreatePeerConversationErrorV1::ConfirmationRefused { required });
    }
    request.founding_participant_ids.sort();
    let room_id = request.room_id.clone();
    let profile = request.profile;
    let reply = client
        .execute(CommandV2::CreatePeerConversation {
            room_id: request.room_id,
            name: request.name,
            profile: request.profile,
            founding_participant_ids: request.founding_participant_ids,
            expected_revision: request.expected_revision,
        })
        .map_err(CreatePeerConversationErrorV1::Client)?;
    match reply {
        ReplyV2::Accepted(OutcomeV2::PeerConversationCreated {
            revision,
            profile: accepted_profile,
        }) if accepted_profile == profile => Ok(CreatePeerConversationReceiptV1 {
            room_id,
            revision,
            profile,
        }),
        ReplyV2::Refused(refusal) => Err(CreatePeerConversationErrorV1::Refused(refusal)),
        ReplyV2::Accepted(_) => Err(CreatePeerConversationErrorV1::UnexpectedReply),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v2::{PortFailureV2, ServicePortV2};
    use agora_protocol::v2::{ClientIdV2, RequestV2};
    use std::collections::VecDeque;

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
            self.replies.pop_front().expect("fixture reply")
        }
    }

    fn request() -> CreatePeerConversationRequestV1 {
        CreatePeerConversationRequestV1 {
            room_id: "task-17".into(),
            name: "Task · task-17".into(),
            profile: ConversationProfileV1::Direct,
            founding_participant_ids: vec!["operator".into(), "local-agent".into()],
            expected_revision: 4,
        }
    }

    #[test]
    fn exact_confirmation_precedes_one_direct_conversation_command() {
        let mut client = ClientV2::new(
            ClientIdV2(7),
            Port {
                requests: Vec::new(),
                replies: VecDeque::from([Ok(ReplyV2::Accepted(
                    OutcomeV2::PeerConversationCreated {
                        revision: 5,
                        profile: ConversationProfileV1::Direct,
                    },
                ))]),
            },
        );
        let required = required_peer_conversation_confirmation(&request());
        let receipt = create_peer_conversation(&mut client, request(), &required).unwrap();
        assert_eq!(receipt.room_id, "task-17");
        assert_eq!(receipt.revision, 5);
        assert_eq!(receipt.profile, ConversationProfileV1::Direct);
        let (port, pending) = client.into_parts();
        assert!(pending.is_none());
        assert_eq!(port.requests.len(), 1);
        assert!(matches!(
            &port.requests[0].command,
            CommandV2::CreatePeerConversation {
                room_id,
                profile: ConversationProfileV1::Direct,
                founding_participant_ids,
                expected_revision: 4,
                ..
            } if room_id == "task-17"
                && founding_participant_ids == &["local-agent", "operator"]
        ));
    }

    #[test]
    fn wrong_confirmation_sends_nothing() {
        let mut client = ClientV2::new(
            ClientIdV2(8),
            Port {
                requests: Vec::new(),
                replies: VecDeque::new(),
            },
        );
        assert!(matches!(
            create_peer_conversation(&mut client, request(), "yes"),
            Err(CreatePeerConversationErrorV1::ConfirmationRefused { .. })
        ));
        let (port, pending) = client.into_parts();
        assert!(pending.is_none());
        assert!(port.requests.is_empty());
    }
}
