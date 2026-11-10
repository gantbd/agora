# Roadmap

1. Freeze canonical V2 checkpoint and event fixtures after the first consumer
   migration; add cross-version refusal fixtures.
2. Define a transport-neutral protocol crate without importing authentication or
   orchestration semantics into the engine.
3. Qualify the Moira receiver against participant-neutral addressed messages and
   lost-reply recovery.
4. Add bounded observation using a separately qualified Keystone mechanism.
5. Add model, fuzz, and mutation gates around recovery and command refusal.
6. Specify moderation, message revision, threads, and deletion only as separately
   bounded profiles with explicit authority and retention consequences.

Agent harness selection, prompting, retries, and execution remain out of scope.

