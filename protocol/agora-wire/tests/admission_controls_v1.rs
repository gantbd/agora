use agora_protocol::v2::{
    AdmissionControlStatusV1, ClientIdV2, CommandV2, MessageIdV1, MessageReportDispositionV1,
    MessageReportIdV1, MessageReportV1, OutcomeV2, RefusalV2, ReplyV2, RequestIdV2, RequestV2,
};
use agora_wire::WireLimits;
use agora_wire::v2::{
    WireReplyV2, WireRequestV2, decode_reply_v2, decode_request_v2, encode_reply_v2,
    encode_request_v2,
};

fn execute(command: CommandV2) -> WireRequestV2 {
    WireRequestV2::Execute {
        client: ClientIdV2(3),
        request: RequestV2 {
            request_id: RequestIdV2(5),
            command,
        },
    }
}

fn decode_hex(source: &str) -> Vec<u8> {
    source
        .split_whitespace()
        .map(|pair| u8::from_str_radix(pair, 16).unwrap())
        .collect()
}

fn report(disposition: MessageReportDispositionV1) -> MessageReportV1 {
    MessageReportV1 {
        report_id: MessageReportIdV1(7),
        room_id: "room".to_owned(),
        message_id: MessageIdV1::new([3; 16]),
        original_sequence: 4,
        target_author_id: "human".to_owned(),
        target_availability: agora_protocol::v2::StableReferenceAvailabilityV1::Available,
        reporter_id: "agent".to_owned(),
        reason: "unsafe".to_owned(),
        causation: [4; 16],
        creation_order: 7,
        disposition,
        review_reason: (disposition != MessageReportDispositionV1::Pending)
            .then(|| "manual review".to_owned()),
        review_causation: (disposition != MessageReportDispositionV1::Pending).then_some([5; 16]),
    }
}

#[test]
fn private_report_commands_and_outcomes_round_trip_canonically() {
    for request in [
        execute(CommandV2::MessageReports {
            room_id: "room".to_owned(),
        }),
        execute(CommandV2::ModerationReports {
            room_id: "room".to_owned(),
        }),
        execute(CommandV2::SubmitMessageReport {
            room_id: "room".to_owned(),
            observed_revision: 11,
            message_id: MessageIdV1::new([3; 16]),
            reason: "unsafe".to_owned(),
            causation: [4; 16],
        }),
        execute(CommandV2::ReviewMessageReport {
            room_id: "room".to_owned(),
            report_id: MessageReportIdV1(7),
            disposition: MessageReportDispositionV1::Escalated,
            reason: "manual review".to_owned(),
            causation: [5; 16],
        }),
    ] {
        let bytes = encode_request_v2(&request, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_request_v2(&bytes, WireLimits::reference()).unwrap(),
            request
        );
    }
    for reply in [
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageReports(vec![report(
            MessageReportDispositionV1::Pending,
        )]))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageReportSubmitted(
            report(MessageReportDispositionV1::Pending),
        ))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageReportReviewed(report(
            MessageReportDispositionV1::Dismissed,
        )))),
        WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::InvalidMessageReportChange)),
    ] {
        let bytes = encode_reply_v2(&reply, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_reply_v2(&bytes, WireLimits::reference()).unwrap(),
            reply
        );
    }
}

#[test]
fn admission_and_private_report_frames_are_frozen() {
    let request = execute(CommandV2::SetPostingRestricted {
        room_id: "room".to_owned(),
        participant_id: "agent".to_owned(),
        restricted: true,
        reason: "cooldown".to_owned(),
        causation: [7; 16],
        expected_revision: 9,
    });
    assert_eq!(
        encode_request_v2(&request, WireLimits::reference()).unwrap(),
        decode_hex(include_str!(
            "fixtures/admission-restriction-request-v2.hex"
        ))
    );
    let reply = WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageReports(vec![report(
        MessageReportDispositionV1::Pending,
    )])));
    assert_eq!(
        encode_reply_v2(&reply, WireLimits::reference()).unwrap(),
        decode_hex(include_str!("fixtures/message-report-list-reply-v2.hex"))
    );
}

#[test]
fn admission_control_commands_round_trip_canonically() {
    for request in [
        execute(CommandV2::AdmissionControl {
            room_id: "room".to_owned(),
        }),
        execute(CommandV2::SetMinimumMessageInterval {
            room_id: "room".to_owned(),
            minimum_interval_ms: 250,
            expected_revision: 9,
        }),
        execute(CommandV2::SetPostingRestricted {
            room_id: "room".to_owned(),
            participant_id: "agent".to_owned(),
            restricted: true,
            reason: "cooldown".to_owned(),
            causation: [7; 16],
            expected_revision: 9,
        }),
    ] {
        let bytes = encode_request_v2(&request, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_request_v2(&bytes, WireLimits::reference()).unwrap(),
            request
        );
    }
}

#[test]
fn private_status_and_precise_refusals_round_trip_canonically() {
    for reply in [
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::AdmissionControl(
            AdmissionControlStatusV1 {
                revision: 11,
                room_id: "room".to_owned(),
                minimum_message_interval_ms: 250,
                observer_posting_restricted: true,
            },
        ))),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::MessageIntervalChanged {
            revision: 12,
        })),
        WireReplyV2::Executed(ReplyV2::Accepted(OutcomeV2::PostingRestrictionChanged {
            revision: 13,
        })),
        WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::PostingRestricted)),
        WireReplyV2::Executed(ReplyV2::Refused(RefusalV2::MessageThrottled {
            retry_after_ms: 37,
        })),
    ] {
        let bytes = encode_reply_v2(&reply, WireLimits::reference()).unwrap();
        assert_eq!(
            decode_reply_v2(&bytes, WireLimits::reference()).unwrap(),
            reply
        );
    }
}
