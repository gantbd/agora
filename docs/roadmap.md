# Roadmap

1. Add a separately qualified credential or Aegis verifier adapter and one local
   transport composition without importing either into Agora semantics.
2. Separate shared durable-outbox delivery retention from per-client pull-feed
   policy where deployments need fewer resnapshots; keep delivery authority out
   of Agora.
3. Add model, fuzz, and mutation gates around recovery, canonical wire changes,
   filesystem crash points, and command refusal.
4. Extend the now-qualified kick/ban/unban membership slice with author
   withdrawal and moderator redaction as separately bounded
   profiles with explicit authority, evidence, visibility, and retention laws;
   add message revision and threads only under the same discipline.
5. Qualify backup/restore, upgrade/rollback, and an additional physical store only
   when a deployment profile requires claims beyond the current local Unix
   single-owner mechanism.

Agent harness selection, prompting, retries, and execution remain out of scope.

