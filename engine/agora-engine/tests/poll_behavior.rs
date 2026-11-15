use agora_engine::persistence::RecoveryLimits;
use agora_engine::{
    AdmissionPolicy, CommandV2, DomainEventV2, Engine, Limits, MAX_POLLS_PER_ROOM_V1, OutcomeV2,
    Participant, ParticipantId, ParticipantKind, PollObservationRefusalV1, Refusal, Role, RoomId,
};

fn participant(id: &str, kind: ParticipantKind) -> Participant {
    Participant::new(id, id, kind)
}

fn room_with_members() -> (Engine, RoomId, ParticipantId, ParticipantId, ParticipantId) {
    let mut engine = Engine::new();
    let alice = ParticipantId::from("alice");
    let bot = ParticipantId::from("bot");
    let moderator = ParticipantId::from("moderator");
    let room = RoomId::from("room");
    for value in [
        participant("alice", ParticipantKind::Human),
        participant("bot", ParticipantKind::Agent),
        participant("moderator", ParticipantKind::Human),
    ] {
        engine.apply(CommandV2::RegisterParticipant(value)).unwrap();
    }
    engine
        .apply(CommandV2::CreateRoom {
            room_id: room.clone(),
            name: "room".to_owned(),
            owner: alice.clone(),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for member in [&bot, &moderator] {
        engine
            .apply(CommandV2::Join {
                room_id: room.clone(),
                participant: member.clone(),
            })
            .unwrap();
    }
    engine
        .apply(CommandV2::SetRole {
            room_id: room.clone(),
            actor: alice.clone(),
            participant: moderator.clone(),
            role: Role::Moderator,
        })
        .unwrap();
    (engine, room, alice, bot, moderator)
}

fn create_poll(
    engine: &mut Engine,
    room: &RoomId,
    actor: &ParticipantId,
    eligible: Vec<ParticipantId>,
) -> agora_engine::PollId {
    match engine
        .apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: actor.clone(),
            prompt: "Choose".to_owned(),
            options: vec!["one".to_owned(), "two".to_owned(), "three".to_owned()],
            eligible_participants: eligible,
            max_choices: 2,
        })
        .unwrap()
    {
        OutcomeV2::PollCreated { poll_id, .. } => poll_id,
        other => panic!("unexpected outcome: {other:?}"),
    }
}

#[test]
fn ballots_are_private_aggregated_replaceable_and_lifecycle_fenced() {
    let (mut engine, room, alice, bot, moderator) = room_with_members();
    let poll_id = create_poll(&mut engine, &room, &alice, vec![alice.clone(), bot.clone()]);

    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: bot.clone(),
            poll_id,
            selections: vec![0, 2],
        })
        .unwrap();
    let status = engine.poll(&moderator, &room, poll_id).unwrap();
    assert_eq!(status.totals(), &[1, 0, 1]);
    assert_eq!(status.total_ballots(), 1);
    assert_eq!(
        engine
            .poll_ballot(&bot, &room, poll_id)
            .unwrap()
            .selections(),
        &[0, 2]
    );
    assert!(
        engine
            .poll_ballot(&alice, &room, poll_id)
            .unwrap()
            .selections()
            .is_empty()
    );
    assert_eq!(
        engine.poll_ballot(&moderator, &room, poll_id),
        Err(PollObservationRefusalV1::NotEligible)
    );
    assert!(matches!(
        &engine.pending_events().last().unwrap().event,
        DomainEventV2::PollBallotChanged {
            totals,
            total_ballots: 1,
            ..
        } if totals == &[1, 0, 1]
    ));

    engine
        .apply(CommandV2::SetPollOpen {
            room_id: room.clone(),
            actor: moderator.clone(),
            poll_id,
            open: false,
        })
        .unwrap();
    assert_eq!(
        engine.apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: bot.clone(),
            poll_id,
            selections: vec![1],
        }),
        Err(Refusal::PollClosed)
    );
    engine
        .apply(CommandV2::SetPollOpen {
            room_id: room.clone(),
            actor: alice.clone(),
            poll_id,
            open: true,
        })
        .unwrap();
    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: bot.clone(),
            poll_id,
            selections: vec![],
        })
        .unwrap();
    let status = engine.poll(&alice, &room, poll_id).unwrap();
    assert_eq!(status.totals(), &[0, 0, 0]);
    assert_eq!(status.total_ballots(), 0);
}

#[test]
fn invalid_poll_inputs_and_duplicate_ballots_are_atomic() {
    let (mut engine, room, alice, bot, _) = room_with_members();
    let revision = engine.revision();
    let events = engine.pending_events().len();
    assert_eq!(
        engine.apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: alice.clone(),
            prompt: "Choose".to_owned(),
            options: vec!["same".to_owned(), "same".to_owned()],
            eligible_participants: vec![alice.clone(), bot.clone()],
            max_choices: 1,
        }),
        Err(Refusal::InvalidPollOptions)
    );
    assert_eq!(engine.revision(), revision);
    assert_eq!(engine.pending_events().len(), events);

    let poll_id = create_poll(&mut engine, &room, &alice, vec![alice.clone(), bot.clone()]);
    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: bot.clone(),
            poll_id,
            selections: vec![1],
        })
        .unwrap();
    let revision = engine.revision();
    assert_eq!(
        engine.apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: bot,
            poll_id,
            selections: vec![1],
        }),
        Err(Refusal::InvalidPollBallotChange)
    );
    assert_eq!(engine.revision(), revision);
}

#[test]
fn poll_state_and_private_ballots_recover_canonically() {
    let (mut engine, room, alice, bot, _) = room_with_members();
    let poll_id = create_poll(&mut engine, &room, &alice, vec![alice.clone(), bot]);
    engine
        .apply(CommandV2::SetPollBallot {
            room_id: room.clone(),
            actor: alice.clone(),
            poll_id,
            selections: vec![0, 2],
        })
        .unwrap();
    let checkpoint = engine.prepare_checkpoint(4 * 1024 * 1024).unwrap();
    assert_eq!(checkpoint.bytes()[8], 151);
    let recovered = Engine::recover_checkpoint(
        checkpoint.bytes(),
        RecoveryLimits {
            max_snapshot_bytes: 4 * 1024 * 1024,
            maximum_engine_limits: Limits::default(),
        },
    )
    .unwrap();
    assert_eq!(
        recovered
            .poll_ballot(&alice, &room, poll_id)
            .unwrap()
            .selections(),
        &[0, 2]
    );
    assert_eq!(
        recovered.poll(&alice, &room, poll_id).unwrap().totals(),
        &[1, 0, 1]
    );
    assert_eq!(
        recovered
            .prepare_checkpoint(4 * 1024 * 1024)
            .unwrap()
            .bytes(),
        checkpoint.bytes()
    );

    for index in 0..checkpoint.bytes().len() {
        for bit in 0..8 {
            let mut mutated = checkpoint.bytes().to_vec();
            mutated[index] ^= 1 << bit;
            if let Ok(candidate) = Engine::recover_checkpoint(
                &mutated,
                RecoveryLimits {
                    max_snapshot_bytes: 4 * 1024 * 1024,
                    maximum_engine_limits: Limits::default(),
                },
            ) {
                assert_eq!(
                    candidate
                        .prepare_checkpoint(4 * 1024 * 1024)
                        .unwrap()
                        .bytes(),
                    mutated,
                    "accepted mutation must already be canonical at byte {index} bit {bit}"
                );
            }
        }
    }
}

#[test]
fn capacity_refuses_with_only_open_polls_and_evicts_the_oldest_closed_poll() {
    let (mut engine, room, alice, _, _) = room_with_members();
    let mut first = None;
    for index in 0..MAX_POLLS_PER_ROOM_V1 {
        let poll_id = match engine
            .apply(CommandV2::CreatePoll {
                room_id: room.clone(),
                actor: alice.clone(),
                prompt: format!("poll-{index}"),
                options: vec!["no".to_owned(), "yes".to_owned()],
                eligible_participants: vec![alice.clone()],
                max_choices: 1,
            })
            .unwrap()
        {
            OutcomeV2::PollCreated { poll_id, .. } => poll_id,
            other => panic!("unexpected outcome: {other:?}"),
        };
        first.get_or_insert(poll_id);
    }
    assert_eq!(
        engine.apply(CommandV2::CreatePoll {
            room_id: room.clone(),
            actor: alice.clone(),
            prompt: "overflow".to_owned(),
            options: vec!["no".to_owned(), "yes".to_owned()],
            eligible_participants: vec![alice.clone()],
            max_choices: 1,
        }),
        Err(Refusal::PollLimit)
    );
    let first = first.unwrap();
    engine
        .apply(CommandV2::SetPollOpen {
            room_id: room.clone(),
            actor: alice.clone(),
            poll_id: first,
            open: false,
        })
        .unwrap();
    create_poll(&mut engine, &room, &alice, vec![alice.clone()]);
    assert_eq!(
        engine.poll(&alice, &room, first),
        Err(PollObservationRefusalV1::UnknownPoll)
    );
    assert_eq!(
        engine.polls(&alice, &room).unwrap().len(),
        MAX_POLLS_PER_ROOM_V1
    );
}
