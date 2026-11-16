//! Additive local transport routing for conversation and transient frames.

use std::fmt;

const SESSION_HELLO_MAGIC_V1: [u8; 8] = *b"AGORATE1";
pub const LOCAL_SESSION_HELLO_BYTES_V1: usize = 40;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalRouteV1 {
    Conversation,
    Transient,
    Search,
    Scheduled,
    Control,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalRouteErrorV1 {
    Empty,
    Limit,
    UnknownRoute,
    InvalidSessionHello,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct LocalReplayEpochV1([u8; 32]);

impl LocalReplayEpochV1 {
    pub fn new(bytes: [u8; 32]) -> Option<Self> {
        (bytes != [0; 32]).then_some(Self(bytes))
    }

    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

impl fmt::Debug for LocalReplayEpochV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("LocalReplayEpochV1(<opaque>)")
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct LocalRoutedFrameV1<'a> {
    route: LocalRouteV1,
    payload: &'a [u8],
}

impl<'a> LocalRoutedFrameV1<'a> {
    pub const fn route(&self) -> LocalRouteV1 {
        self.route
    }

    pub const fn payload(&self) -> &'a [u8] {
        self.payload
    }
}

impl fmt::Debug for LocalRoutedFrameV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LocalRoutedFrameV1")
            .field("route", &self.route)
            .field("payload_bytes", &self.payload.len())
            .finish()
    }
}

pub fn encode_local_routed_frame_v1(
    route: LocalRouteV1,
    payload: &[u8],
    max_frame_bytes: usize,
) -> Result<Vec<u8>, LocalRouteErrorV1> {
    let length = payload
        .len()
        .checked_add(1)
        .ok_or(LocalRouteErrorV1::Limit)?;
    if payload.is_empty() || max_frame_bytes == 0 || length > max_frame_bytes {
        return Err(if payload.is_empty() {
            LocalRouteErrorV1::Empty
        } else {
            LocalRouteErrorV1::Limit
        });
    }
    let mut encoded = Vec::new();
    encoded
        .try_reserve_exact(length)
        .map_err(|_| LocalRouteErrorV1::Limit)?;
    encoded.push(route_tag(route));
    encoded.extend_from_slice(payload);
    Ok(encoded)
}

pub fn decode_local_routed_frame_v1(
    input: &[u8],
    max_frame_bytes: usize,
) -> Result<LocalRoutedFrameV1<'_>, LocalRouteErrorV1> {
    if input.len() < 2 {
        return Err(LocalRouteErrorV1::Empty);
    }
    if max_frame_bytes == 0 || input.len() > max_frame_bytes {
        return Err(LocalRouteErrorV1::Limit);
    }
    let route = match input[0] {
        1 => LocalRouteV1::Conversation,
        2 => LocalRouteV1::Transient,
        3 => LocalRouteV1::Control,
        4 => LocalRouteV1::Search,
        5 => LocalRouteV1::Scheduled,
        _ => return Err(LocalRouteErrorV1::UnknownRoute),
    };
    Ok(LocalRoutedFrameV1 {
        route,
        payload: &input[1..],
    })
}

pub fn encode_local_session_hello_v1(
    replay_epoch: LocalReplayEpochV1,
) -> [u8; LOCAL_SESSION_HELLO_BYTES_V1] {
    let mut encoded = [0; LOCAL_SESSION_HELLO_BYTES_V1];
    encoded[..8].copy_from_slice(&SESSION_HELLO_MAGIC_V1);
    encoded[8..].copy_from_slice(&replay_epoch.0);
    encoded
}

pub fn decode_local_session_hello_v1(
    input: &[u8],
) -> Result<LocalReplayEpochV1, LocalRouteErrorV1> {
    if input.len() != LOCAL_SESSION_HELLO_BYTES_V1 || input[..8] != SESSION_HELLO_MAGIC_V1 {
        return Err(LocalRouteErrorV1::InvalidSessionHello);
    }
    LocalReplayEpochV1::new(
        input[8..]
            .try_into()
            .expect("session hello has exact epoch bytes"),
    )
    .ok_or(LocalRouteErrorV1::InvalidSessionHello)
}

fn route_tag(route: LocalRouteV1) -> u8 {
    match route {
        LocalRouteV1::Conversation => 1,
        LocalRouteV1::Transient => 2,
        LocalRouteV1::Control => 3,
        LocalRouteV1::Search => 4,
        LocalRouteV1::Scheduled => 5,
    }
}
