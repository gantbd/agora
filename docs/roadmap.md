# Roadmap

1. Add cross-version and malformed compatibility fixtures around the now-frozen
   canonical V2 checkpoint fixture.
2. Qualify the Moira receiver against participant-neutral addressed messages and
   lost-reply recovery.
3. Add a separately qualified credential or Aegis verifier adapter and one local
   transport composition without importing either into Agora semantics.
4. Add a separately qualified durable store adapter with atomic engine and
   service-receipt recovery semantics.
5. Add bounded observation using a separately qualified Keystone mechanism.
6. Add model, fuzz, and mutation gates around recovery and command refusal.
7. Specify moderation, message revision, threads, and deletion only as separately
   bounded profiles with explicit authority and retention consequences.

Agent harness selection, prompting, retries, and execution remain out of scope.

