//! Canonical fixed-width handshake for the qualified private local transport.
//!
//! The profile negotiates framing facts before authentication. It grants no
//! authority and defines no socket, credential, retry, or lifecycle policy.

use std::fmt;

const CLIENT_HELLO_MAGIC_V1: [u8; 8] = *b"AGORAHC1";
const SERVER_HELLO_MAGIC_V1: [u8; 8] = *b"AGORAHS1";
const HANDSHAKE_PROFILE_V1: u16 = 1;
const WIRE_PROFILE_V2: u16 = 2;
const FEATURE_DISCOVERY_PROFILE_V1: u32 = 1;
const COLLABORATION_CLIENT_HELLO_MAGIC_V1: [u8; 8] = *b"AGORAHC2";
const COLLABORATION_SERVER_HELLO_MAGIC_V1: [u8; 8] = *b"AGORAHS2";
const COLLABORATION_HANDSHAKE_PROFILE_V1: u16 = 2;
const TRANSIENT_WIRE_PROFILE_V1: u16 = 1;
pub const CLIENT_HELLO_BYTES_V1: usize = 12;
pub const SERVER_HELLO_BYTES_V1: usize = 40;
pub const COLLABORATION_CLIENT_HELLO_BYTES_V1: usize = 14;
pub const COLLABORATION_SERVER_HELLO_BYTES_V1: usize = 42;

/// Caller-supplied identity for one logical server incarnation.
///
/// This value is transport evidence, not participant identity, authority, or
/// an ordering coordinate. The all-zero value is not encodable.
#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct ServerIncarnationV1([u8; 16]);

impl ServerIncarnationV1 {
    pub const fn new(bytes: [u8; 16]) -> Self {
        Self(bytes)
    }

    pub const fn as_bytes(self) -> [u8; 16] {
        self.0
    }

    pub fn is_zero(self) -> bool {
        self.0 == [0; 16]
    }
}

impl fmt::Debug for ServerIncarnationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ServerIncarnationV1([redacted; 16])")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HandshakeErrorV1 {
    Length,
    Magic,
    Profile,
    Limits,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ServerHelloV1 {
    max_frame_bytes: u32,
    max_frames_after_connect: u32,
    server_incarnation: ServerIncarnationV1,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct CollaborationServerHelloV1 {
    max_frame_bytes: u32,
    max_frames_after_connect: u32,
    server_incarnation: ServerIncarnationV1,
}

impl CollaborationServerHelloV1 {
    pub const fn max_frame_bytes(self) -> u32 {
        self.max_frame_bytes
    }

    pub const fn max_frames_after_connect(self) -> u32 {
        self.max_frames_after_connect
    }

    pub const fn server_incarnation(self) -> ServerIncarnationV1 {
        self.server_incarnation
    }

    pub const fn supports_capability_discovery(self) -> bool {
        true
    }

    pub const fn supports_transient_collaboration(self) -> bool {
        true
    }
}

impl fmt::Debug for CollaborationServerHelloV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CollaborationServerHelloV1")
            .field("handshake_profile", &COLLABORATION_HANDSHAKE_PROFILE_V1)
            .field("conversation_wire_profile", &WIRE_PROFILE_V2)
            .field("transient_wire_profile", &TRANSIENT_WIRE_PROFILE_V1)
            .field("max_frame_bytes", &self.max_frame_bytes)
            .field("max_frames_after_connect", &self.max_frames_after_connect)
            .field("server_incarnation_present", &true)
            .field(
                "capability_discovery_profile",
                &FEATURE_DISCOVERY_PROFILE_V1,
            )
            .finish()
    }
}

impl ServerHelloV1 {
    pub const fn max_frame_bytes(self) -> u32 {
        self.max_frame_bytes
    }

    pub const fn max_frames_after_connect(self) -> u32 {
        self.max_frames_after_connect
    }

    pub const fn server_incarnation(self) -> ServerIncarnationV1 {
        self.server_incarnation
    }

    pub const fn supports_capability_discovery(self) -> bool {
        true
    }
}

impl fmt::Debug for ServerHelloV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ServerHelloV1")
            .field("handshake_profile", &HANDSHAKE_PROFILE_V1)
            .field("wire_profile", &WIRE_PROFILE_V2)
            .field("max_frame_bytes", &self.max_frame_bytes)
            .field("max_frames_after_connect", &self.max_frames_after_connect)
            .field("server_incarnation_present", &true)
            .field(
                "capability_discovery_profile",
                &FEATURE_DISCOVERY_PROFILE_V1,
            )
            .finish()
    }
}

pub const fn encode_client_hello_v1() -> [u8; CLIENT_HELLO_BYTES_V1] {
    let mut bytes = [0_u8; CLIENT_HELLO_BYTES_V1];
    let mut index = 0;
    while index < CLIENT_HELLO_MAGIC_V1.len() {
        bytes[index] = CLIENT_HELLO_MAGIC_V1[index];
        index += 1;
    }
    let handshake = HANDSHAKE_PROFILE_V1.to_be_bytes();
    bytes[8] = handshake[0];
    bytes[9] = handshake[1];
    let wire = WIRE_PROFILE_V2.to_be_bytes();
    bytes[10] = wire[0];
    bytes[11] = wire[1];
    bytes
}

pub fn decode_client_hello_v1(input: &[u8]) -> Result<(), HandshakeErrorV1> {
    if input.len() != CLIENT_HELLO_BYTES_V1 {
        return Err(HandshakeErrorV1::Length);
    }
    if input[..8] != CLIENT_HELLO_MAGIC_V1 {
        return Err(HandshakeErrorV1::Magic);
    }
    if u16::from_be_bytes([input[8], input[9]]) != HANDSHAKE_PROFILE_V1
        || u16::from_be_bytes([input[10], input[11]]) != WIRE_PROFILE_V2
    {
        return Err(HandshakeErrorV1::Profile);
    }
    Ok(())
}

pub fn encode_server_hello_v1(
    server_incarnation: ServerIncarnationV1,
    max_frame_bytes: u32,
    max_frames_after_connect: u32,
) -> Result<[u8; SERVER_HELLO_BYTES_V1], HandshakeErrorV1> {
    if max_frame_bytes == 0 || max_frames_after_connect == 0 || server_incarnation.is_zero() {
        return Err(HandshakeErrorV1::Limits);
    }
    let mut bytes = [0_u8; SERVER_HELLO_BYTES_V1];
    bytes[..8].copy_from_slice(&SERVER_HELLO_MAGIC_V1);
    bytes[8..10].copy_from_slice(&HANDSHAKE_PROFILE_V1.to_be_bytes());
    bytes[10..12].copy_from_slice(&WIRE_PROFILE_V2.to_be_bytes());
    bytes[12..16].copy_from_slice(&max_frame_bytes.to_be_bytes());
    bytes[16..20].copy_from_slice(&max_frames_after_connect.to_be_bytes());
    bytes[20..36].copy_from_slice(&server_incarnation.0);
    bytes[36..40].copy_from_slice(&FEATURE_DISCOVERY_PROFILE_V1.to_be_bytes());
    Ok(bytes)
}

pub fn decode_server_hello_v1(input: &[u8]) -> Result<ServerHelloV1, HandshakeErrorV1> {
    if input.len() != SERVER_HELLO_BYTES_V1 {
        return Err(HandshakeErrorV1::Length);
    }
    if input[..8] != SERVER_HELLO_MAGIC_V1 {
        return Err(HandshakeErrorV1::Magic);
    }
    if u16::from_be_bytes([input[8], input[9]]) != HANDSHAKE_PROFILE_V1
        || u16::from_be_bytes([input[10], input[11]]) != WIRE_PROFILE_V2
        || u32::from_be_bytes(input[36..40].try_into().expect("four-byte feature profile"))
            != FEATURE_DISCOVERY_PROFILE_V1
    {
        return Err(HandshakeErrorV1::Profile);
    }
    let max_frame_bytes = u32::from_be_bytes(input[12..16].try_into().expect("four-byte limit"));
    let max_frames_after_connect =
        u32::from_be_bytes(input[16..20].try_into().expect("four-byte budget"));
    let server_incarnation = ServerIncarnationV1(
        input[20..36]
            .try_into()
            .expect("sixteen-byte server incarnation"),
    );
    if max_frame_bytes == 0 || max_frames_after_connect == 0 || server_incarnation.is_zero() {
        return Err(HandshakeErrorV1::Limits);
    }
    Ok(ServerHelloV1 {
        max_frame_bytes,
        max_frames_after_connect,
        server_incarnation,
    })
}

pub const fn encode_collaboration_client_hello_v1() -> [u8; COLLABORATION_CLIENT_HELLO_BYTES_V1] {
    let mut bytes = [0_u8; COLLABORATION_CLIENT_HELLO_BYTES_V1];
    let mut index = 0;
    while index < COLLABORATION_CLIENT_HELLO_MAGIC_V1.len() {
        bytes[index] = COLLABORATION_CLIENT_HELLO_MAGIC_V1[index];
        index += 1;
    }
    let handshake = COLLABORATION_HANDSHAKE_PROFILE_V1.to_be_bytes();
    bytes[8] = handshake[0];
    bytes[9] = handshake[1];
    let conversation = WIRE_PROFILE_V2.to_be_bytes();
    bytes[10] = conversation[0];
    bytes[11] = conversation[1];
    let transient = TRANSIENT_WIRE_PROFILE_V1.to_be_bytes();
    bytes[12] = transient[0];
    bytes[13] = transient[1];
    bytes
}

pub fn decode_collaboration_client_hello_v1(input: &[u8]) -> Result<(), HandshakeErrorV1> {
    if input.len() != COLLABORATION_CLIENT_HELLO_BYTES_V1 {
        return Err(HandshakeErrorV1::Length);
    }
    if input[..8] != COLLABORATION_CLIENT_HELLO_MAGIC_V1 {
        return Err(HandshakeErrorV1::Magic);
    }
    if u16::from_be_bytes([input[8], input[9]]) != COLLABORATION_HANDSHAKE_PROFILE_V1
        || u16::from_be_bytes([input[10], input[11]]) != WIRE_PROFILE_V2
        || u16::from_be_bytes([input[12], input[13]]) != TRANSIENT_WIRE_PROFILE_V1
    {
        return Err(HandshakeErrorV1::Profile);
    }
    Ok(())
}

pub fn encode_collaboration_server_hello_v1(
    server_incarnation: ServerIncarnationV1,
    max_frame_bytes: u32,
    max_frames_after_connect: u32,
) -> Result<[u8; COLLABORATION_SERVER_HELLO_BYTES_V1], HandshakeErrorV1> {
    if max_frame_bytes == 0 || max_frames_after_connect == 0 || server_incarnation.is_zero() {
        return Err(HandshakeErrorV1::Limits);
    }
    let mut bytes = [0; COLLABORATION_SERVER_HELLO_BYTES_V1];
    bytes[..8].copy_from_slice(&COLLABORATION_SERVER_HELLO_MAGIC_V1);
    bytes[8..10].copy_from_slice(&COLLABORATION_HANDSHAKE_PROFILE_V1.to_be_bytes());
    bytes[10..12].copy_from_slice(&WIRE_PROFILE_V2.to_be_bytes());
    bytes[12..14].copy_from_slice(&TRANSIENT_WIRE_PROFILE_V1.to_be_bytes());
    bytes[14..18].copy_from_slice(&max_frame_bytes.to_be_bytes());
    bytes[18..22].copy_from_slice(&max_frames_after_connect.to_be_bytes());
    bytes[22..38].copy_from_slice(&server_incarnation.0);
    bytes[38..42].copy_from_slice(&FEATURE_DISCOVERY_PROFILE_V1.to_be_bytes());
    Ok(bytes)
}

pub fn decode_collaboration_server_hello_v1(
    input: &[u8],
) -> Result<CollaborationServerHelloV1, HandshakeErrorV1> {
    if input.len() != COLLABORATION_SERVER_HELLO_BYTES_V1 {
        return Err(HandshakeErrorV1::Length);
    }
    if input[..8] != COLLABORATION_SERVER_HELLO_MAGIC_V1 {
        return Err(HandshakeErrorV1::Magic);
    }
    if u16::from_be_bytes([input[8], input[9]]) != COLLABORATION_HANDSHAKE_PROFILE_V1
        || u16::from_be_bytes([input[10], input[11]]) != WIRE_PROFILE_V2
        || u16::from_be_bytes([input[12], input[13]]) != TRANSIENT_WIRE_PROFILE_V1
        || u32::from_be_bytes(input[38..42].try_into().expect("four-byte feature profile"))
            != FEATURE_DISCOVERY_PROFILE_V1
    {
        return Err(HandshakeErrorV1::Profile);
    }
    let max_frame_bytes = u32::from_be_bytes(input[14..18].try_into().expect("four-byte limit"));
    let max_frames_after_connect =
        u32::from_be_bytes(input[18..22].try_into().expect("four-byte budget"));
    let server_incarnation = ServerIncarnationV1(
        input[22..38]
            .try_into()
            .expect("sixteen-byte server incarnation"),
    );
    if max_frame_bytes == 0 || max_frames_after_connect == 0 || server_incarnation.is_zero() {
        return Err(HandshakeErrorV1::Limits);
    }
    Ok(CollaborationServerHelloV1 {
        max_frame_bytes,
        max_frames_after_connect,
        server_incarnation,
    })
}
