use agora_protocol::v2::{
    CapabilityV2, ClientIdV2, CommandV2, OutcomeV2, PageCursorV2, PollBallotV1, PollIdV1,
    PollStatusV1, RefusalV2, ReplyV2, RequestIdV2, RequestV2, RoomChangePageV2, RoomChangeV2,
    RoomEventV2,
};
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};
use agora_wire::{WireEncodeError, WireLimits};

fn poll_id() -> PollIdV1 {
    PollIdV1::new([7; 16])
}

fn status() -> PollStatusV1 {
    PollStatusV1 {
        poll_id: poll_id(),
        creator_id: "human".to_owned(),
        prompt: "choose privately".to_owned(),
        options: vec!["alpha".to_owned(), "beta".to_owned()],
        eligible_participant_ids: vec!["agent".to_owned(), "human".to_owned()],
        max_choices: 1,
        open: true,
        totals: vec![1, 0],
        total_ballots: 1,
    }
}

#[test]
fn poll_vocabulary_round_trips_canonically() {
    let commands = vec![
        CommandV2::Polls {
            room_id: "room".to_owned(),
        },
        CommandV2::Poll {
            room_id: "room".to_owned(),
            poll_id: poll_id(),
        },
        CommandV2::PollBallot {
            room_id: "room".to_owned(),
            poll_id: poll_id(),
        },
        CommandV2::CreatePoll {
            room_id: "room".to_owned(),
            expected_revision: 4,
            prompt: "choose privately".to_owned(),
            options: vec!["alpha".to_owned(), "beta".to_owned()],
            eligible_participant_ids: vec!["agent".to_owned(), "human".to_owned()],
            max_choices: 1,
        },
        CommandV2::SetPollBallot {
            room_id: "room".to_owned(),
            expected_revision: 5,
            poll_id: poll_id(),
            selections: vec![1],
        },
        CommandV2::SetPollOpen {
            room_id: "room".to_owned(),
            expected_revision: 6,
            poll_id: poll_id(),
            open: false,
        },
    ];
    for (index, command) in commands.into_iter().enumerate() {
        let value = WireRequestV2::Execute {
            client: ClientIdV2(1),
            request: RequestV2 {
                request_id: RequestIdV2(index as u64 + 1),
                command,
            },
        };
        let bytes = encode_request_v2(&value, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_request_v2(&bytes, WireLimits::reference()).unwrap(),
            value
        );
    }

    let outcomes = vec![
        OutcomeV2::Capabilities(vec![CapabilityV2::BoundedPollsV1]),
        OutcomeV2::Polls(vec![status()]),
        OutcomeV2::Poll(status()),
        OutcomeV2::PollBallot(PollBallotV1 {
            poll_id: poll_id(),
            selections: vec![1],
        }),
        OutcomeV2::PollCreated {
            revision: 5,
            poll_id: poll_id(),
        },
        OutcomeV2::PollBallotChanged {
            revision: 6,
            poll_id: poll_id(),
            totals: vec![0, 1],
            total_ballots: 1,
        },
        OutcomeV2::PollLifecycleChanged {
            revision: 7,
            poll_id: poll_id(),
            open: false,
        },
        OutcomeV2::Changes(RoomChangePageV2 {
            revision: 5,
            changes: vec![RoomChangeV2 {
                event_sequence: 3,
                revision: 5,
                event: RoomEventV2::PollCreated {
                    actor_id: "human".to_owned(),
                    poll_id: poll_id(),
                    prompt: "choose privately".to_owned(),
                    options: vec!["alpha".to_owned(), "beta".to_owned()],
                    eligible_participant_ids: vec!["agent".to_owned(), "human".to_owned()],
                    max_choices: 1,
                },
            }],
            next: PageCursorV2(3),
            has_more: false,
        }),
    ];
    for outcome in outcomes {
        let value = WireReplyV2::Executed(ReplyV2::Accepted(outcome));
        let bytes = encode_reply_v2(&value, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_reply_v2(&bytes, WireLimits::reference()).unwrap(),
            value
        );
    }

    for refusal in [
        RefusalV2::InvalidPoll,
        RefusalV2::PollLimit,
        RefusalV2::UnknownPoll,
        RefusalV2::NotPollEligible,
        RefusalV2::PollClosed,
        RefusalV2::InvalidPollChange,
    ] {
        let value = WireReplyV2::Executed(ReplyV2::Refused(refusal));
        let bytes = encode_reply_v2(&value, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_reply_v2(&bytes, WireLimits::reference()).unwrap(),
            value
        );
    }
}

#[test]
fn poll_encoding_rejects_noncanonical_private_selections() {
    let value = WireRequestV2::Execute {
        client: ClientIdV2(1),
        request: RequestV2 {
            request_id: RequestIdV2(1),
            command: CommandV2::SetPollBallot {
                room_id: "room".to_owned(),
                expected_revision: 5,
                poll_id: poll_id(),
                selections: vec![1, 1],
            },
        },
    };
    assert_eq!(
        encode_request_v2(&value, WireLimits::reference()),
        Err(WireEncodeError::Invalid)
    );
}

#[test]
fn private_ballot_debug_does_not_expose_selections() {
    let ballot = PollBallotV1 {
        poll_id: poll_id(),
        selections: vec![3, 9],
    };
    let debug = format!("{ballot:?}");
    assert!(debug.contains("selection_count"));
    assert!(!debug.contains("3"));
    assert!(!debug.contains("9"));
}
