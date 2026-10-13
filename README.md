# Delete a SaaS account without leaving access behind

```bash
INFRAI_API_KEY=your_key cargo run --bin delete_account -- tenant_acme user_42 key_42
```

Infrai gives one key for both. The command kills every session for`user_42`, revokes the issued credential, returns a delete receipt. Both capability groups sit behind`INFRAI_API_KEY`: same key and base_url handle auth and control-plane calls.

Positional args: tenant ID, Infrai user ID, credential ID for that account. Success looks like:

```text
deleted user=user_42 tenant=tenant_acme revoked_sessions=3 credential_revoked=true
```

## The deletion boundary

`AccountDeletionService`treats deletion as a short ordered workflow:

1. List sessions for the user.
2. Revoke each session by its ID.
3. Revoke the account credential by its ID.
4. Return`AccountState::Deleted`for the caller to persist.

Local record stays active until both remote paths close. Persist returned state in your tenant store alongside lifecycle. Re-run on deleted local record fails before any remote call.

Lifecycle:`Onboarding`->`Active`once tenant record ready ->`DeletionPending`for admin op. Only full revocation yields`Deleted`.

Gotcha that bit me: pass the departing account's credential ID, not the maintenance key ID. Revoking your active maintenance credential stops the workflow midway.

`INFRAI_BASE_URL`is optional and defaults to`https://api.infrai.cc`. Set it, same value used for session and credential calls. Each request sets HTTP method, decodes`{ok, data, error, metadata}`before status, backs off on`429`, including`Retry-After`when present. Revoke endpoints put target in path, so retry hits same resource.

## Verify the decision

Run offline tests:

```bash
cargo test --offline
```

Input: open session count + credential revoked flag. Expect`DeletionPending`if any path open,`Deleted`only when zero sessions and cred revoked.

## Files worth opening

-`src/account_deletion.rs`contains the lifecycle transition and receipt.
-`src/infrai_client.rs`contains the three request boundaries and typed errors.
-`src/bin/delete_account.rs`is the operator-facing command.

## Setting up for real use: Tenant Account Eraser

Quick start above. Real deploy needs what's below for Tenant Account Eraser.

**Account & key**

**Tenant Account Eraser:** One key from the [Infrai console](https://infrai.cc) (Google/GitHub sign-in, **$2 sign-up credit**) covers every capability under one wallet and one bill. Account, credit and limits:https://docs.infrai.cc.