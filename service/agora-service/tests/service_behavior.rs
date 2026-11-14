use agora_engine::{AdmissionPolicy, CommandV2, Engine, Participant, ParticipantKind, Revision};
use agora_protocol::{
    ClientIdV1, CommandV1, OutcomeV1, ParticipantKindV1, RefusalV1, ReplyV1, RequestIdV1,
    RequestV1, RoleV1,
};
use agora_service::{Service, ServiceLimits};

fn request(id: u64, command: CommandV1) -> RequestV1 {
    RequestV1 {
        request_id: RequestIdV1(id),
        command,
    }
}

fn snapshot(id: u64) -> RequestV1 {
    request(
        id,
        CommandV1::Snapshot {
            room_id: "general".to_owned(),
        },
    )
}

fn speak(id: u64, revision: u64, body: &str, addressed_to: Vec<&str>) -> RequestV1 {
    request(
        id,
        CommandV1::Speak {
            room_id: "general".to_owned(),
            expected_revision: revision,
            logical_time: 7,
            body: body.to_owned(),
            addressed_to: addressed_to.into_iter().map(str::to_owned).collect(),
        },
    )
}

fn engine() -> Engine {
    let mut engine = Engine::new();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "alice",
            "Alice",
            ParticipantKind::Human,
        )))
        .unwrap();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "bot",
            "Bot",
            ParticipantKind::Agent,
        )))
        .unwrap();
    engine
        .apply(CommandV2::CreateRoom {
            room_id: "general".into(),
            name: "General".to_owned(),
            owner: "alice".into(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    engine
        .apply(CommandV2::Join {
            room_id: "general".into(),
            participant: "bot".into(),
        })
        .unwrap();
    engine
}

#[test]
fn clients_are_bound_to_participants_without_kind_privilege() {
    let mut service = Service::new(engine(), ServiceLimits::default());
    let human = service.connect("alice").unwrap();
    let agent = service.connect("bot").unwrap();

    let first = service.execute(agent, speak(0, 4, "status", vec!["alice"]));
    assert_eq!(
        first,
        ReplyV1::Accepted(OutcomeV1::MessageAccepted {
            revision: 5,
            sequence: 0,
        })
    );
    let ReplyV1::Accepted(OutcomeV1::Snapshot(snapshot)) = service.execute(human, snapshot(0))
    else {
        panic!("snapshot should be accepted");
    };
    assert_eq!(snapshot.revision, 5);
    assert_eq!(snapshot.messages[0].author, "bot");
    assert_eq!(snapshot.messages[0].addressed_to, ["alice"]);
    assert_eq!(
        snapshot
            .members
            .iter()
            .map(|member| (&*member.participant_id, member.kind, member.role))
            .collect::<Vec<_>>(),
        [
            ("alice", ParticipantKindV1::Human, RoleV1::Owner),
            ("bot", ParticipantKindV1::Agent, RoleV1::Member),
        ]
    );
}

#[test]
fn matching_request_replays_and_changed_request_conflicts() {
    let mut service = Service::new(engine(), ServiceLimits::default());
    let client = service.connect("alice").unwrap();
    let original = speak(9, 4, "one", vec![]);

    let accepted = service.execute(client, original.clone());
    assert_eq!(service.execute(client, original), accepted);
    assert_eq!(service.engine().revision(), Revision(5));
    assert_eq!(
        service.execute(client, speak(9, 5, "changed", vec![])),
        ReplyV1::Refused(RefusalV1::RequestConflict)
    );
    assert_eq!(service.engine().revision(), Revision(5));
    assert_eq!(service.usage().replayed, 1);
    assert_eq!(service.usage().conflicts, 1);
}

#[test]
fn limits_and_validation_refuse_before_engine_mutation() {
    let limits = ServiceLimits {
        max_retained_requests: 1,
        ..ServiceLimits::default()
    };
    let mut service = Service::new(engine(), limits);
    let client = service.connect("alice").unwrap();
    assert!(matches!(
        service.execute(client, snapshot(0)),
        ReplyV1::Accepted(OutcomeV1::Snapshot(_))
    ));
    assert_eq!(
        service.execute(client, speak(1, 4, "blocked", vec![])),
        ReplyV1::Refused(RefusalV1::RequestLimit)
    );
    assert_eq!(service.engine().revision(), Revision(4));

    let mut service = Service::new(engine(), ServiceLimits::default());
    let client = service.connect("alice").unwrap();
    assert_eq!(
        service.execute(client, speak(0, 4, "bad order", vec!["bot", "alice"])),
        ReplyV1::Refused(RefusalV1::InvalidInput)
    );
    assert_eq!(service.engine().revision(), Revision(4));

    let mut service = Service::new(
        engine(),
        ServiceLimits {
            max_retained_requests: 1,
            max_participant_id_bytes: 5,
            ..ServiceLimits::default()
        },
    );
    let client = service.connect("alice").unwrap();
    assert_eq!(
        service.execute(client, speak(0, 4, "invalid", vec!["too-long"])),
        ReplyV1::Refused(RefusalV1::InvalidInput)
    );
    assert!(matches!(
        service.execute(client, snapshot(1)),
        ReplyV1::Accepted(OutcomeV1::Snapshot(_))
    ));
}

#[test]
fn disconnect_is_tombstoned_and_reclaims_request_receipts() {
    let limits = ServiceLimits {
        max_clients: 1,
        max_retained_requests: 1,
        ..ServiceLimits::default()
    };
    let mut service = Service::new(engine(), limits);
    let first = service.connect("alice").unwrap();
    assert_eq!(first, ClientIdV1(0));
    service.execute(first, snapshot(0));
    assert_eq!(service.disconnect(first), Ok(true));
    assert_eq!(service.disconnect(first), Ok(false));
    assert_eq!(
        service.execute(first, snapshot(1)),
        ReplyV1::Refused(RefusalV1::UnknownClient)
    );

    let second = service.connect("bot").unwrap();
    assert_eq!(second, ClientIdV1(1));
    assert!(matches!(
        service.execute(second, snapshot(0)),
        ReplyV1::Accepted(OutcomeV1::Snapshot(_))
    ));
}

#[test]
fn tombstone_pressure_never_blocks_disconnect() {
    let limits = ServiceLimits {
        max_clients: 2,
        max_retired_clients: 0,
        ..ServiceLimits::default()
    };
    let mut service = Service::new(engine(), limits);
    let client = service.connect("alice").unwrap();
    assert_eq!(service.disconnect(client), Ok(true));
    assert_eq!(service.disconnect(client), Err(RefusalV1::UnknownClient));

    let limits = ServiceLimits {
        max_clients: 2,
        max_retired_clients: 1,
        ..ServiceLimits::default()
    };
    let mut service = Service::new(engine(), limits);
    let first = service.connect("alice").unwrap();
    assert_eq!(service.disconnect(first), Ok(true));
    let second = service.connect("bot").unwrap();
    assert_eq!(service.disconnect(second), Ok(true));
    assert_eq!(service.disconnect(first), Err(RefusalV1::UnknownClient));
    assert_eq!(service.disconnect(second), Ok(false));
}

#[test]
fn snapshots_do_not_cross_membership_boundary() {
    let mut engine = engine();
    engine
        .apply(CommandV2::RegisterParticipant(Participant::new(
            "outsider",
            "Outsider",
            ParticipantKind::Human,
        )))
        .unwrap();
    let mut service = Service::new(engine, ServiceLimits::default());
    let outsider = service.connect("outsider").unwrap();
    assert_eq!(
        service.execute(outsider, snapshot(0)),
        ReplyV1::Refused(RefusalV1::NotMember)
    );
}
