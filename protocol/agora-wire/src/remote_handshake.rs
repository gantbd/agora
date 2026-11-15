//! Canonical post-TLS negotiation for the bounded remote transport profile.
//!
//! TLS establishes transport peer provenance and confidentiality before these
//! bytes are exchanged. This vocabulary grants no participant identity,
//! authority, session resumption, or federation trust.

use crate::local_handshake::ServerIncarnationV1;
use std::fmt;

const CLIENT_MAGIC_V1: [u8; 8] = *b"AGORARC1";
const SERVER_MAGIC_V1: [u8; 8] = *b"AGORARS1";
pub const REMOTE_ALPN_V1: &[u8] = b"agora/remote/1";
pub const CLIENT_HELLO_BYTES_V1: usize = 20;
pub const SERVER_HELLO_BYTES_V1: usize = 44;
pub const HANDSHAKE_PROFILE_V1: u16 = 1;
pub const CONVERSATION_WIRE_PROFILE_V2: u16 = 2;
pub const CAPABILITY_DISCOVERY_PROFILE_V1: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProfileRangeV1 {
    pub minimum: u16,
    pub maximum: u16,
}

impl ProfileRangeV1 {
    pub const fn new(minimum: u16, maximum: u16) -> Result<Self, RemoteHandshakeErrorV1> {
        if minimum == 0 || minimum > maximum {
            return Err(RemoteHandshakeErrorV1::Profile);
        }
        Ok(Self { minimum, maximum })
    }

    pub const fn select_highest(self, supported: Self) -> Option<u16> {
        let minimum = if self.minimum > supported.minimum {
            self.minimum
        } else {
            supported.minimum
        };
        let maximum = if self.maximum < supported.maximum {
            self.maximum
        } else {
            supported.maximum
        };
        if minimum <= maximum {
            Some(maximum)
        } else {
            None
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RemoteClientHelloV1 {
    pub handshake_profiles: ProfileRangeV1,
    pub conversation_wire_profiles: ProfileRangeV1,
}

impl RemoteClientHelloV1 {
    pub const fn current() -> Self {
        Self {
            handshake_profiles: ProfileRangeV1 {
                minimum: HANDSHAKE_PROFILE_V1,
                maximum: HANDSHAKE_PROFILE_V1,
            },
            conversation_wire_profiles: ProfileRangeV1 {
                minimum: CONVERSATION_WIRE_PROFILE_V2,
                maximum: CONVERSATION_WIRE_PROFILE_V2,
            },
        }
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct RemoteServerHelloV1 {
    pub handshake_profile: u16,
    pub conversation_wire_profile: u16,
    pub max_frame_bytes: u32,
    pub max_requests: u32,
    pub max_pending_writes: u32,
    pub server_incarnation: ServerIncarnationV1,
}

impl fmt::Debug for RemoteServerHelloV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RemoteServerHelloV1")
            .field("handshake_profile", &self.handshake_profile)
            .field("conversation_wire_profile", &self.conversation_wire_profile)
            .field("max_frame_bytes", &self.max_frame_bytes)
            .field("max_requests", &self.max_requests)
            .field("max_pending_writes", &self.max_pending_writes)
            .field("server_incarnation_present", &true)
            .field(
                "capability_discovery_profile",
                &CAPABILITY_DISCOVERY_PROFILE_V1,
            )
            .finish()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RemoteHandshakeErrorV1 {
    Length,
    Magic,
    Profile,
    Limits,
}

pub fn encode_client_hello_v1(hello: RemoteClientHelloV1) -> [u8; CLIENT_HELLO_BYTES_V1] {
    let mut bytes = [0; CLIENT_HELLO_BYTES_V1];
    bytes[..8].copy_from_slice(&CLIENT_MAGIC_V1);
    bytes[8..10].copy_from_slice(&hello.handshake_profiles.minimum.to_be_bytes());
    bytes[10..12].copy_from_slice(&hello.handshake_profiles.maximum.to_be_bytes());
    bytes[12..14].copy_from_slice(&hello.conversation_wire_profiles.minimum.to_be_bytes());
    bytes[14..16].copy_from_slice(&hello.conversation_wire_profiles.maximum.to_be_bytes());
    bytes[16..20].copy_from_slice(&CAPABILITY_DISCOVERY_PROFILE_V1.to_be_bytes());
    bytes
}

pub fn decode_client_hello_v1(bytes: &[u8]) -> Result<RemoteClientHelloV1, RemoteHandshakeErrorV1> {
    if bytes.len() != CLIENT_HELLO_BYTES_V1 {
        return Err(RemoteHandshakeErrorV1::Length);
    }
    if bytes[..8] != CLIENT_MAGIC_V1 {
        return Err(RemoteHandshakeErrorV1::Magic);
    }
    if u32::from_be_bytes(bytes[16..20].try_into().expect("fixed feature field"))
        != CAPABILITY_DISCOVERY_PROFILE_V1
    {
        return Err(RemoteHandshakeErrorV1::Profile);
    }
    Ok(RemoteClientHelloV1 {
        handshake_profiles: ProfileRangeV1::new(
            u16::from_be_bytes([bytes[8], bytes[9]]),
            u16::from_be_bytes([bytes[10], bytes[11]]),
        )?,
        conversation_wire_profiles: ProfileRangeV1::new(
            u16::from_be_bytes([bytes[12], bytes[13]]),
            u16::from_be_bytes([bytes[14], bytes[15]]),
        )?,
    })
}

pub fn negotiate_server_hello_v1(
    client: RemoteClientHelloV1,
    supported_handshake_profiles: ProfileRangeV1,
    supported_wire_profiles: ProfileRangeV1,
    max_frame_bytes: u32,
    max_requests: u32,
    max_pending_writes: u32,
    server_incarnation: ServerIncarnationV1,
) -> Result<RemoteServerHelloV1, RemoteHandshakeErrorV1> {
    if max_frame_bytes == 0
        || max_requests == 0
        || max_pending_writes == 0
        || server_incarnation.is_zero()
    {
        return Err(RemoteHandshakeErrorV1::Limits);
    }
    Ok(RemoteServerHelloV1 {
        handshake_profile: client
            .handshake_profiles
            .select_highest(supported_handshake_profiles)
            .ok_or(RemoteHandshakeErrorV1::Profile)?,
        conversation_wire_profile: client
            .conversation_wire_profiles
            .select_highest(supported_wire_profiles)
            .ok_or(RemoteHandshakeErrorV1::Profile)?,
        max_frame_bytes,
        max_requests,
        max_pending_writes,
        server_incarnation,
    })
}

pub fn encode_server_hello_v1(
    hello: RemoteServerHelloV1,
) -> Result<[u8; SERVER_HELLO_BYTES_V1], RemoteHandshakeErrorV1> {
    if hello.handshake_profile == 0
        || hello.conversation_wire_profile == 0
        || hello.max_frame_bytes == 0
        || hello.max_requests == 0
        || hello.max_pending_writes == 0
        || hello.server_incarnation.is_zero()
    {
        return Err(RemoteHandshakeErrorV1::Limits);
    }
    let mut bytes = [0; SERVER_HELLO_BYTES_V1];
    bytes[..8].copy_from_slice(&SERVER_MAGIC_V1);
    bytes[8..10].copy_from_slice(&hello.handshake_profile.to_be_bytes());
    bytes[10..12].copy_from_slice(&hello.conversation_wire_profile.to_be_bytes());
    bytes[12..16].copy_from_slice(&hello.max_frame_bytes.to_be_bytes());
    bytes[16..20].copy_from_slice(&hello.max_requests.to_be_bytes());
    bytes[20..24].copy_from_slice(&hello.max_pending_writes.to_be_bytes());
    bytes[24..40].copy_from_slice(&hello.server_incarnation.as_bytes());
    bytes[40..44].copy_from_slice(&CAPABILITY_DISCOVERY_PROFILE_V1.to_be_bytes());
    Ok(bytes)
}

pub fn decode_server_hello_v1(bytes: &[u8]) -> Result<RemoteServerHelloV1, RemoteHandshakeErrorV1> {
    if bytes.len() != SERVER_HELLO_BYTES_V1 {
        return Err(RemoteHandshakeErrorV1::Length);
    }
    if bytes[..8] != SERVER_MAGIC_V1 {
        return Err(RemoteHandshakeErrorV1::Magic);
    }
    if u32::from_be_bytes(bytes[40..44].try_into().expect("fixed feature field"))
        != CAPABILITY_DISCOVERY_PROFILE_V1
    {
        return Err(RemoteHandshakeErrorV1::Profile);
    }
    let hello = RemoteServerHelloV1 {
        handshake_profile: u16::from_be_bytes([bytes[8], bytes[9]]),
        conversation_wire_profile: u16::from_be_bytes([bytes[10], bytes[11]]),
        max_frame_bytes: u32::from_be_bytes(bytes[12..16].try_into().expect("fixed frame field")),
        max_requests: u32::from_be_bytes(bytes[16..20].try_into().expect("fixed request field")),
        max_pending_writes: u32::from_be_bytes(
            bytes[20..24].try_into().expect("fixed pending field"),
        ),
        server_incarnation: ServerIncarnationV1::new(
            bytes[24..40].try_into().expect("fixed incarnation field"),
        ),
    };
    encode_server_hello_v1(hello)?;
    Ok(hello)
}
