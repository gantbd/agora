# Security policy

## Supported versions

Agora has not made a production release. The `main` branch is the only line
currently considered for security fixes; historical snapshots and untagged
source archives are unsupported.

## Reporting a vulnerability

Do not disclose a suspected vulnerability in a public issue, discussion, pull
request, chat room, fixture, or transcript. Use GitHub private vulnerability
reporting for this repository when it is enabled. If it is unavailable, contact
the repository owner privately through their GitHub profile and provide only
enough information to establish a confidential channel.

Include the affected revision, impact, preconditions, a minimal reproduction,
and whether credentials, participant identity, private-room membership, message
content, addressing, read state, or other metadata may have been exposed. Do not
attach real user data, credentials, tokens, or unredacted private transcripts.

Receipt of a report is not a promise of a response or remediation deadline.
Coordinated disclosure timing will be agreed after the report is reproduced and
its scope is understood.

## Security boundary

Agora is a conversation engine and transport-neutral host substrate, not an
authentication provider, policy engine, agent sandbox, encrypted messenger, or
workflow executor. Host applications must establish principal identity and
authority, protect transports and durable records, and prevent addressed
messages from becoming implicit agent execution. The concrete
[threat and privacy model](docs/threat-model.md) defines current boundaries and
residual risks. [Release policy](docs/release-policy.md) defines distribution
limits.
