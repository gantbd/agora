use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationIdV1, ScheduledPublicationIntentV1,
    ScheduledPublicationObservationRefusalV1, ScheduledPublicationObservationV1,
    ScheduledPublicationRefusalV1, ScheduledPublicationRequestV1,
};
use agora_wire::scheduled::{
    ScheduledWireReplyV1, ScheduledWireRequestV1, decode_scheduled_reply_v1,
    decode_scheduled_request_v1, encode_scheduled_reply_v1, encode_scheduled_request_v1,
};
use agora_wire::{WireDecodeError, WireLimits};

fn limits() -> WireLimits {
    WireLimits {
        max_frame_bytes: 512,
        max_text_bytes: 64,
        max_fields: 32,
        max_list_items: 4,
    }
}

fn request() -> ScheduledPublicationRequestV1 {
    ScheduledPublicationRequestV1 {
        id: ScheduledPublicationIdV1::new([7; 16]).unwrap(),
        intent: ScheduledPublicationIntentV1::new("room", 19, "private body"),
    }
}

fn actor_request() -> ActorPublicationRequestV1 {
    ActorPublicationRequestV1 {
        id: ActorPublicationIdV1::new([8; 16]).unwrap(),
        intent: ActorPublicationIntentV1::new(
            "room",
            11,
            20,
            "private actor body",
            vec!["human".into()],
        ),
    }
}

#[test]
fn scheduled_requests_are_closed_bounded_and_content_safe() {
    for request in [
        ScheduledWireRequestV1::Publish(request()),
        ScheduledWireRequestV1::Observe(request()),
        ScheduledWireRequestV1::ProjectDelivery(DeliverySourceV1 { event_sequence: 41 }),
        ScheduledWireRequestV1::PublishActor(actor_request()),
        ScheduledWireRequestV1::ObserveActor(actor_request()),
        ScheduledWireRequestV1::ObserveActorProvenance(actor_request()),
    ] {
        let bytes = encode_scheduled_request_v1(&request, limits()).unwrap();
        assert_eq!(decode_scheduled_request_v1(&bytes, limits()), Ok(request));
    }
    let debug = format!("{:?}", ScheduledWireRequestV1::Observe(request()));
    assert!(!debug.contains("private body"));
    let actor_debug = format!(
        "{:?}",
        ScheduledWireRequestV1::ObserveActor(actor_request())
    );
    assert!(!actor_debug.contains("private actor body"));
    assert!(!actor_debug.contains("human"));
    let mut zero =
        encode_scheduled_request_v1(&ScheduledWireRequestV1::Observe(request()), limits()).unwrap();
    zero[2..18].fill(0);
    assert_eq!(
        decode_scheduled_request_v1(&zero, limits()),
        Err(WireDecodeError::Malformed)
    );
}

#[test]
fn scheduled_replies_round_trip_every_semantic_class() {
    let accepted = ScheduledPublicationAcceptedV1 {
        revision: 11,
        message_sequence: 3,
    };
    let replies = [
        ScheduledWireReplyV1::Published(Ok(accepted)),
        ScheduledWireReplyV1::Published(Err(ScheduledPublicationRefusalV1::AdmissionDeferred {
            retry_after_ms: 8,
        })),
        ScheduledWireReplyV1::Observed(Ok(ScheduledPublicationObservationV1::Accepted(accepted))),
        ScheduledWireReplyV1::Observed(Ok(ScheduledPublicationObservationV1::Absent)),
        ScheduledWireReplyV1::Observed(Err(ScheduledPublicationObservationRefusalV1::Conflict)),
        ScheduledWireReplyV1::DeliveryProjected(Ok(DeliveryProjectionV1::Visible(
            WakeProjectionV1 {
                source: DeliverySourceV1 { event_sequence: 41 },
                class: WakeClassV1::ConversationActivity,
            },
        ))),
        ScheduledWireReplyV1::DeliveryProjected(Ok(DeliveryProjectionV1::NotVisible)),
        ScheduledWireReplyV1::DeliveryProjected(Ok(DeliveryProjectionV1::Gap { retained_from: 9 })),
        ScheduledWireReplyV1::DeliveryProjected(Ok(DeliveryProjectionV1::Unavailable)),
        ScheduledWireReplyV1::DeliveryProjected(Err(DeliveryProjectionRefusalV1::NotAuthorized)),
        ScheduledWireReplyV1::DeliveryProjected(Err(DeliveryProjectionRefusalV1::Unavailable)),
        ScheduledWireReplyV1::ActorPublished(Ok(ActorPublicationAcceptedV1 {
            revision: 12,
            message_id: [9; 16],
            message_sequence: 4,
        })),
        ScheduledWireReplyV1::ActorPublished(Err(ActorPublicationRefusalV1::AdmissionDeferred {
            retry_after_ms: 9,
        })),
        ScheduledWireReplyV1::ActorObserved(Ok(ActorPublicationObservationV1::Accepted(
            ActorPublicationAcceptedV1 {
                revision: 12,
                message_id: [9; 16],
                message_sequence: 4,
            },
        ))),
        ScheduledWireReplyV1::ActorObserved(Ok(ActorPublicationObservationV1::Absent)),
        ScheduledWireReplyV1::ActorObserved(Err(ActorPublicationObservationRefusalV1::Conflict)),
        ScheduledWireReplyV1::ActorProvenanceObserved(Ok(
            ActorPublicationProvenanceObservationV1::Accepted(ActorPublicationProvenanceV1 {
                accepted: ActorPublicationAcceptedV1 {
                    revision: 12,
                    message_id: [9; 16],
                    message_sequence: 4,
                },
                actor: "agent".into(),
            }),
        )),
        ScheduledWireReplyV1::ActorProvenanceObserved(Ok(
            ActorPublicationProvenanceObservationV1::Absent,
        )),
        ScheduledWireReplyV1::ActorProvenanceObserved(Err(
            ActorPublicationObservationRefusalV1::Conflict,
        )),
    ];
    for reply in replies {
        let bytes = encode_scheduled_reply_v1(&reply, limits()).unwrap();
        assert_eq!(decode_scheduled_reply_v1(&bytes, limits()), Ok(reply));
    }
}
use agora_actor_publication_port::{
    ActorPublicationAcceptedV1, ActorPublicationIdV1, ActorPublicationIntentV1,
    ActorPublicationObservationRefusalV1, ActorPublicationObservationV1,
    ActorPublicationProvenanceObservationV1, ActorPublicationProvenanceV1,
    ActorPublicationRefusalV1, ActorPublicationRequestV1,
};
use agora_delivery_projection_port::{
    DeliveryProjectionRefusalV1, DeliveryProjectionV1, DeliverySourceV1, WakeClassV1,
    WakeProjectionV1,
};
