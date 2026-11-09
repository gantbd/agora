use agora_engine::{
    AdmissionPolicy, AgentActivationPolicy, CommandV1, DomainEventV1, Engine, LogicalTime,
    Participant, ParticipantId, ParticipantKind, Refusal, RoomId,
};

fn setup_room() -> Engine {
    let mut engine = Engine::new();
    for (id, kind) in [
        ("alice", ParticipantKind::Human),
        ("bot-a", ParticipantKind::Agent),
        ("bot-b", ParticipantKind::Agent),
    ] {
        engine
            .apply(CommandV1::RegisterParticipant(Participant::new(
                id, id, kind,
            )))
            .unwrap();
    }
    engine
        .apply(CommandV1::CreateRoom {
            room_id: RoomId::from("room"),
            name: "Room".into(),
            owner: ParticipantId::from("alice"),
            admission: AdmissionPolicy::Open,
        })
        .unwrap();
    for agent in ["bot-a", "bot-b"] {
        engine
            .apply(CommandV1::Join {
                room_id: RoomId::from("room"),
                participant: ParticipantId::from(agent),
            })
            .unwrap();
    }
    engine
}

fn speak(engine: &mut Engine, author: &str, body: &str) {
    engine
        .apply(CommandV1::Speak {
            room_id: RoomId::from("room"),
            author: ParticipantId::from(author),
            at: LogicalTime(1),
            body: body.into(),
        })
        .unwrap();
}

#[test]
fn human_messages_activate_agent_members_in_stable_order() {
    let mut engine = setup_room();
    engine
        .apply(CommandV1::SetAgentActivationPolicy {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("alice"),
            policy: AgentActivationPolicy::OnHumanMessage,
        })
        .unwrap();
    speak(&mut engine, "alice", "please help");

    let event = &engine.pending_events().last().unwrap().event;
    let DomainEventV1::MessageAccepted {
        activate_agents, ..
    } = event
    else {
        panic!("expected message event");
    };
    assert_eq!(
        activate_agents,
        &vec![ParticipantId::from("bot-a"), ParticipantId::from("bot-b")]
    );
}

#[test]
fn agent_messages_never_recursively_activate_agents() {
    let mut engine = setup_room();
    engine
        .apply(CommandV1::SetAgentActivationPolicy {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("alice"),
            policy: AgentActivationPolicy::OnHumanMessage,
        })
        .unwrap();
    speak(&mut engine, "bot-a", "agent response");

    let DomainEventV1::MessageAccepted {
        activate_agents, ..
    } = &engine.pending_events().last().unwrap().event
    else {
        panic!("expected message event");
    };
    assert!(activate_agents.is_empty());
}

#[test]
fn explicit_activation_requires_a_human_member_and_retained_cause() {
    let mut engine = setup_room();
    engine
        .apply(CommandV1::SetAgentActivationPolicy {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("alice"),
            policy: AgentActivationPolicy::Explicit,
        })
        .unwrap();
    speak(&mut engine, "alice", "question");

    assert_eq!(
        engine.apply(CommandV1::RequestAgentActivation {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("bot-a"),
            agent: ParticipantId::from("bot-b"),
            cause_message_sequence: 0,
        }),
        Err(Refusal::ActivationRequesterMustBeHuman)
    );
    engine
        .apply(CommandV1::RequestAgentActivation {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("alice"),
            agent: ParticipantId::from("bot-a"),
            cause_message_sequence: 0,
        })
        .unwrap();
    assert!(matches!(
        engine.pending_events().last().unwrap().event,
        DomainEventV1::AgentActivationRequested { .. }
    ));
}

#[test]
fn activation_is_disabled_by_default() {
    let mut engine = setup_room();
    speak(&mut engine, "alice", "question");
    assert_eq!(
        engine.apply(CommandV1::RequestAgentActivation {
            room_id: RoomId::from("room"),
            actor: ParticipantId::from("alice"),
            agent: ParticipantId::from("bot-a"),
            cause_message_sequence: 0,
        }),
        Err(Refusal::AgentActivationDisabled)
    );
}
