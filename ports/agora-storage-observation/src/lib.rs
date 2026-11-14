//! Effect-free observation port for physical checkpoint-store owners.
//!
//! This crate carries no filesystem implementation and assigns no semantic
//! meaning to store-specific counters. It lets a composed durable owner copy a
//! passive, content-free physical snapshot without depending on an adapter.

use keystone_storage::CheckpointStore;
use std::fmt::Debug;

/// Reports whether an observation has lost exact counter fidelity.
pub trait ObservationFidelityV1 {
    fn fidelity_lost(&self) -> bool;
}

/// Optional passive observation extension for a physical checkpoint store.
///
/// Implementations must read owner memory only: no I/O, lock operation,
/// validation, retry, or mutation may occur while producing the snapshot.
pub trait ObservableCheckpointStoreV1: CheckpointStore {
    type Observation: Copy + Debug + Eq + ObservationFidelityV1;

    fn physical_observation(&self) -> Self::Observation;
}
