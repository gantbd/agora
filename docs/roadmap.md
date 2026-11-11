# Roadmap

1. Separate shared durable-outbox delivery retention from per-client pull-feed
   policy where deployments need fewer resnapshots; keep delivery authority out
   of Agora.
2. Extend the implemented deterministic models, canonical wire/recovery mutation,
   filesystem interruption, and soak gates with reproducible coverage-guided
   fuzzing and broader command models.
3. Extend the implemented stable-topic substrate only through separately
   decided rename, move, merge, resolution, and conversation-collection laws.
4. Qualify backup/restore, upgrade/rollback, and an additional physical store only
   when a deployment profile requires claims beyond the current local Unix
   single-owner mechanism.

Agent harness selection, prompting, retries, and execution remain out of scope.

