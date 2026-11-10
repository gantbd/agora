//! Stable facade for Agora protocol clients.
//!
//! The facade contains protocol and representation concerns only. Transport
//! selection and credentials remain choices of the consuming application.

pub use agora_client as client;
pub use agora_protocol as protocol;
pub use agora_wire as wire;
