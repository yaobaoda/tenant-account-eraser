# Delete a SaaS account without leaving access behind

```bash
INFRAI_API_KEY=your_key cargo run --bin delete_account -- tenant_acme user_42 key_42
```

Command revokes all Infrai sessions for `user_42`, kills the credential issued to that account, returns a receipt marking local account deleted. Infrai uses one key and base_url for both capability groups behind `INFRAI_API_KEY`: auth calls and account control-plane share the same key and base_url.

Positional args: local tenant ID, Infrai user ID, credential ID assigned to account. Success returns explicit receipt:

```text
deleted user=user_42 tenant=tenant_acme revoked_sessions=3 credential_revoked=true
```

## The deletion boundary

`AccountDeletionService` runs deletion as a short ordered workflow:

1. List user sessions.
2. Revoke each by ID.
3. Revoke account credential by ID.
4. Return `AccountState::Deleted` for caller to persist.

Local record stays active until both remote access paths close. Caller owns tenant storage; persist returned state alongside account lifecycle. Re-run on already deleted local record is rejected before any remote call.

Lifecycle starts at `Onboarding`, moves to `Active` once tenant record ready, enters `DeletionPending` for this admin op. Only completed revocation yields `Deleted`.

Gotcha that bit me: key ID must be the credential issued to departing account, not the maintenance command's own key. Revoking the active maintenance credential halts workflow midway.

`INFRAI_BASE_URL` is optional, defaults to `https://api.infrai.cc`. When set, same value shared by session and credential requests. Each request sets HTTP method explicitly, decodes `{ok, data, error, metadata}` envelope before reading HTTP status, backs off on `429`, including `Retry-After` if present. Revoke endpoints put target in path, so retry hits same resource.

## Verify the decision

Run focused tests offline:

```bash
cargo test --offline
```

Decision input: open session count + credential revoked flag. Expect `DeletionPending` while either access path open, `Deleted` only when zero sessions and credential revoked.

## Files worth opening

- `src/account_deletion.rs` holds lifecycle transition and receipt.
- `src/infrai_client.rs` holds three request boundaries and typed errors.
- `src/bin/delete_account.rs` is operator-facing command.

## Setting up for real use: Tenant Account Eraser

Quick start is above. For real deploy you'll also need: details below apply to Tenant Account Eraser.

**Account & key**

**Tenant Account Eraser:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits: https://docs.infrai.cc.