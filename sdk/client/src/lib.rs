//! Stable remote-client distribution facade for Agora.
//!
//! The facade contains the transport-neutral client, protocol DTOs, and
//! canonical wire representation only. Transport selection, connection
//! lifecycle, authentication credentials, and retry policy remain choices of
//! the consuming application.

pub use agora_client as client;
pub use agora_protocol as protocol;
pub use agora_wire as wire;
