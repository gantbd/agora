//! Public client for Agora's private local composition transport.

#![cfg(unix)]
#![forbid(unsafe_code)]

use agora_client::scheduled::{ScheduledPublicationWireClientV1, ScheduledTransportErrorV1};
use agora_scheduled_publication_port::{
    ScheduledPublicationAcceptedV1, ScheduledPublicationObservationRefusalV1,
    ScheduledPublicationObservationV1, ScheduledPublicationRefusalV1,
    ScheduledPublicationRequestV1,
};
use agora_wire::WireLimits;
use agora_wire::local_handshake::{
    COMPOSITION_SERVER_HELLO_BYTES_V1, decode_composition_server_hello_v1,
    encode_composition_client_hello_v1,
};
use agora_wire::local_transient::{
    LocalRouteV1, decode_local_routed_frame_v1, decode_local_session_hello_v1,
    encode_local_routed_frame_v1,
};
use agora_wire::scheduled::{
    ScheduledWireReplyV1, ScheduledWireRequestV1, decode_scheduled_reply_v1,
    encode_scheduled_request_v1,
};
use agora_wire::v2::{WireReplyV2, WireRequestV2, decode_reply_v2, encode_request_v2};
use std::fs;
use std::io::{Read, Write};
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LocalCallErrorV1 {
    /// The scheduled request frame was not sent.
    NotSent,
    /// The scheduled request frame may have reached the owner.
    OutcomeUnknown,
    /// A reply arrived but did not match the negotiated profile or operation.
    InvalidResponse,
}

impl ScheduledTransportErrorV1 for LocalCallErrorV1 {
    fn definitely_not_sent(&self) -> bool {
        matches!(self, Self::NotSent)
    }
}

pub struct ScheduledLocalClientV1 {
    socket_path: PathBuf,
    participant_id: String,
    limits: WireLimits,
    timeout: Duration,
}

impl ScheduledLocalClientV1 {
    pub fn new(
        socket_path: impl Into<PathBuf>,
        participant_id: impl Into<String>,
        limits: WireLimits,
        timeout: Duration,
    ) -> Self {
        Self {
            socket_path: socket_path.into(),
            participant_id: participant_id.into(),
            limits,
            timeout,
        }
    }

    fn call(
        &self,
        request: ScheduledWireRequestV1,
    ) -> Result<ScheduledWireReplyV1, LocalCallErrorV1> {
        let scheduled = encode_scheduled_request_v1(&request, self.limits)
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        let connect = encode_request_v2(
            &WireRequestV2::Connect {
                participant_id: self.participant_id.clone(),
            },
            self.limits,
        )
        .map_err(|_| LocalCallErrorV1::NotSent)?;
        verify_socket(&self.socket_path).map_err(|_| LocalCallErrorV1::NotSent)?;
        let mut stream =
            UnixStream::connect(&self.socket_path).map_err(|_| LocalCallErrorV1::NotSent)?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.timeout)))
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        stream
            .write_all(&encode_composition_client_hello_v1())
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        let mut hello = [0; COMPOSITION_SERVER_HELLO_BYTES_V1];
        stream
            .read_exact(&mut hello)
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        let hello =
            decode_composition_server_hello_v1(&hello).map_err(|_| LocalCallErrorV1::NotSent)?;
        let negotiated = usize::try_from(hello.max_frame_bytes())
            .map_err(|_| LocalCallErrorV1::NotSent)?
            .min(self.limits.max_frame_bytes);
        write_routed(
            &mut stream,
            LocalRouteV1::Conversation,
            &connect,
            negotiated,
        )
        .map_err(|_| LocalCallErrorV1::NotSent)?;
        let connected = read_routed(&mut stream, LocalRouteV1::Conversation, negotiated)
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        let WireReplyV2::Connected(_client): WireReplyV2 =
            decode_reply_v2(&connected, self.limits).map_err(|_| LocalCallErrorV1::NotSent)?
        else {
            return Err(LocalCallErrorV1::NotSent);
        };
        let control = read_routed(&mut stream, LocalRouteV1::Control, negotiated)
            .map_err(|_| LocalCallErrorV1::NotSent)?;
        decode_local_session_hello_v1(&control).map_err(|_| LocalCallErrorV1::NotSent)?;
        write_routed(&mut stream, LocalRouteV1::Scheduled, &scheduled, negotiated)
            .map_err(|_| LocalCallErrorV1::OutcomeUnknown)?;
        let reply = read_routed(&mut stream, LocalRouteV1::Scheduled, negotiated)
            .map_err(|_| LocalCallErrorV1::OutcomeUnknown)?;
        decode_scheduled_reply_v1(&reply, self.limits)
            .map_err(|_| LocalCallErrorV1::InvalidResponse)
    }
}

impl ScheduledPublicationWireClientV1 for ScheduledLocalClientV1 {
    type Error = LocalCallErrorV1;

    fn publish_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<Result<ScheduledPublicationAcceptedV1, ScheduledPublicationRefusalV1>, Self::Error>
    {
        match self.call(ScheduledWireRequestV1::Publish(request))? {
            ScheduledWireReplyV1::Published(result) => Ok(result),
            ScheduledWireReplyV1::Observed(_) => Err(LocalCallErrorV1::InvalidResponse),
        }
    }

    fn observe_scheduled(
        &mut self,
        request: ScheduledPublicationRequestV1,
    ) -> Result<
        Result<ScheduledPublicationObservationV1, ScheduledPublicationObservationRefusalV1>,
        Self::Error,
    > {
        match self.call(ScheduledWireRequestV1::Observe(request))? {
            ScheduledWireReplyV1::Observed(result) => Ok(result),
            ScheduledWireReplyV1::Published(_) => Err(LocalCallErrorV1::InvalidResponse),
        }
    }
}

fn verify_socket(path: &Path) -> std::io::Result<()> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.file_type().is_socket()
        || metadata.permissions().mode() & 0o777 != 0o600
        || metadata.nlink() != 1
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "private owner socket required",
        ));
    }
    Ok(())
}

fn write_routed(
    stream: &mut UnixStream,
    route: LocalRouteV1,
    payload: &[u8],
    max_frame_bytes: usize,
) -> std::io::Result<()> {
    let frame = encode_local_routed_frame_v1(route, payload, max_frame_bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "frame limit"))?;
    let length = u32::try_from(frame.len())
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidInput, "frame limit"))?;
    stream.write_all(&length.to_be_bytes())?;
    stream.write_all(&frame)?;
    stream.flush()
}

fn read_routed(
    stream: &mut UnixStream,
    expected: LocalRouteV1,
    max_frame_bytes: usize,
) -> std::io::Result<Vec<u8>> {
    let mut length = [0; 4];
    stream.read_exact(&mut length)?;
    let length = usize::try_from(u32::from_be_bytes(length))
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "frame limit"))?;
    if length == 0 || length > max_frame_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "frame limit",
        ));
    }
    let mut frame = vec![0; length];
    stream.read_exact(&mut frame)?;
    let routed = decode_local_routed_frame_v1(&frame, max_frame_bytes)
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "route"))?;
    if routed.route() != expected {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "route",
        ));
    }
    Ok(routed.payload().to_vec())
}
