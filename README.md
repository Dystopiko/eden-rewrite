# `eden-rewrite/redesign`

> An integrated system built specifically for Dystopia (or any Minecraft server community),
  originally to bridge the gap between its Discord guild and Minecraft server.

This branch contains the revised architecture of **Eden System** as its original legacy codebase
was hurriedly implemented for Season 4, and suffered from structural and user experience
limitations. Therefore, this branch surfaced its issues of using Eden from experiences from
players in Dystopia, and its new architecture in **Eden v3** will hopefully last for a long time.

## License

Eden System and its crates are licensed under the [GNU Affero General Public License v3.0](LICENSE.txt),
except for [`eden-signals`](crates/eden-signals) and [`erased-report`](crates/erased-report), which
are all licensed under the [MIT License](crates/eden-signals/LICENSE.txt).

## Issues

1. Players have to **memorize or copy a random 4-word generated code** and message it directly to
   the Eden bot on Discord within a short period of time. This adds friction and confusion for all
   players because they have to read the entire step-by-step process.

2. Eden, since its first version, has no administrative Web UI to handle special account linking
   and manage settings. The leader maintainer of Eden has to do all of the special interventions
   which it will waste time not for the lead maintainer, but for the staff and players.

3. Implementation of Eden v2 was hurriedly developed prior to Season 4 launch, therefore technical
   debt and complexity had been accumulated throughout the codebase.

4. Eden v2 was originally developed to allow Discord guilds outside Dystopia, to add Eden for its
   auto-response features which it adds more complexity of implementing new features.

5. Every HTTP route uses the same shared bearer token, including EdenMC operations and
   administrative settings or member changes. Eden cannot identify the caller, enforce staff roles,
   or limit a compromised server token to Minecraft-only permissions.

6. Session requests ignore the documented `Idempotency-Key` header. Retried requests can repeat
   rate-limit accounting and enqueue duplicate login events instead of returning the original result.

7. Session authorization only checks whether the Minecraft account exists, whether its edition
   matches, and whether guest access is enabled. It cannot enforce maintenance access, community
   bans, blocked networks, or approval of an unfamiliar IP address.

8. Discord membership synchronization is incomplete. Eden does not handle member joins or removals,
   and removing the configured member or contributor role does not remove existing database access.
   Restarts also skip synchronization after guild settings have been initialized.

9. Initial member synchronization is skipped entirely once a guild reaches 1,000 members instead
   of paginating Discord's member endpoint, leaving large communities dependent on manual repair.

10. Background jobs are claimed as `running` before execution without recovering abandoned jobs
    after a crash. Some jobs also commit database effects before Discord delivery, so retries can
    repeatedly fail on existing records while the intended notification is never sent.

11. Account-link cancellation paths enqueue their cleanup inside a transaction but return without
    committing it. A code exposed in the guild, or submitted by an unregistered Discord user, can
    therefore remain valid until it expires even after Eden says it was cancelled.

12. Community-specific behavior remains embedded in the implementation, including Dystopia-only
    account-link messages and novelty auto-response features. These concerns are coupled to the core
    Discord and Minecraft services instead of being optional configuration or extensions.

## New Architecture

In this new architecture, Eden will have both frontend web application (specifically the admin
dashboard), and Rust backend, but the philosophy of bridging gaps between the community's Discord
guild and its Minecraft server still remains.

Unlike previous versions of Eden, the goal of v3 is to ONLY TARGET for one Discord guild, and
not tie any namings back to Dystopia. The community can customize their name to their linking.

### Utilizing NATS

This replaces the custom database-backed job schedulers with NATS. This new version of Eden
will allow for durable event persistence and fault tolerance.

### API Authorization

EdenMC credentials must only authorize Minecraft integration routes. Administrative routes require
an authenticated Discord member whose current guild roles grant the requested permission. Every
administrative mutation must record the actor, action, target, and timestamp for auditing.

### Discord Member Reconciliation

Eden must process member joins, updates, removals, and relevant role changes. Losing membership or
a privileged role must revoke the corresponding access. Startup reconciliation must paginate the
entire guild and repair missed events instead of relying only on the initial gateway payload.

### Reliable Side Effects

API retries and redelivered NATS events must not duplicate state changes. Consumers must persist a
stable operation identifier, make database mutations atomic, and acknowledge an event only after its
required effects are complete. Interrupted work must remain recoverable, including account-link
expiration and Discord notifications that fail after a database write.

### API Errors

API errors should remain the same.
```json
{
    "code": "READONLY_MODE",
    "message": "{EdenAliasName} is currently in read-only mode."
}
```

### General Community Alerts (`POST /alerts`)

Instead of EdenMC sends only one type of alert for every type to the Eden API, we'll allow multiple
types of alert including the original "admin executed command" alert. The only difference with
this new response body for original AEC alert is to add type field.

```json
{
    "type": "mc_executed_command",
    "command": "/kill @a",
    "executor": { .. }
}
```
<small>
    <a href="https://github.com/Dystopiko/eden-rewrite/blob/5acf2c509b77c9548e878e64976b1def07fdaf24/crates/eden-api-types/src/logs/commands.rs#L8-L10">
        This schema is based from the new `/logs/commands` request body in the rewritten version of
        Eden. Only EdenMC authenticated tokens can execute this!
    </a>
</small>

---

### Graceful Shutdown (`ShutdownSignal`)

In Eden v3, all concurrent services (Axum API server, Twilight Discord gateway worker, background
job processing pools) coordinate graceful shutdown using
[`ShutdownSignal`](crates/eden-signals/src/shutdown.rs) from `crates/eden-signals`.

#### Architecture & Usage

- **Watch Channel Broadcast**: `ShutdownSignal` encapsulates a `tokio::sync::watch` channel (`Sender<bool>`), enabling multi-service broadcast notifications when OS termination signals (`SIGINT` / `SIGTERM`) are received.
- **`subscribe().await`**: Asynchronously suspends the calling task until shutdown is triggered via `initiate()`.
- **`run_or_cancelled(future).await`**: Races any asynchronous future against the shutdown signal using `tokio::select!`. Returns `Some(output)` if the future finishes first, or `None` if shutdown was initiated.
- **`result(future).await`**: Specialized helper for futures returning `Result<T, E>`, resolving to `Ok(None)` if interrupted by system shutdown.

---

There are some events that can be distributed to everyone like this one but it requires the member
from the associated token to be a community administrator/staff:
```json
{
    "type": "server_okay",
    "timestamp": "2020-01-01T00:00:00Z"
}
```

### Session Request Process (`POST /minecraft/login`)

When a player attempts to join the Minecraft server, `EdenMC` sends a session request to the Eden
backend to evaluate whether access should be granted or denied based on the following evaluation
steps:

1. If an administrator or staff disables **Enable Access** in settings, all regular players are
   automatically rejected upon join. Only authorized administrators and staff members
   (identified by UUID) are allowed to connect.

2. For players who have linked their Discord account, connection is permitted if:
   - They are not banned from the community by an administrator or staff member.
   - The client's IP address is not explicitly blocked via CIDR security rules made by either the
     administrators or the player.

3. For players who have not linked a Discord account, connection is permitted if:
   - **Guest Access** is enabled in the community's server settings.
   - The client's IP address is not blocked.
   - They are not banned by an administrator or staff member.

4. If a linked member connects from an IP address that does not match their trusted CIDR list
   (and is not blocked), the session is rejected. Eden sends a notification via Discord DM
   prompting the member to approve or reject the new IP address.

5. Only EdenMC tokens can perform this route!

#### Assumptions and Limitations

Eden assumes that:

- Communication between EdenMC and the Eden back is securely authenticated
- Loopback and private subnets must be rejected automatically (to prevent proxy IP trust leaks)
- Untrusted IP verification DMs are rate-limited.
- Ban checks evaluate player UUIDs, linked Discord IDs, and historical IP/CIDR subnets to prevent
  banned users from evading bans via unlinked guest accounts.
- Session requests use an `Idempotency-Key` header and atomic PostgreSQL transactions to prevent
  race conditions during rapid reconnect attempts.
- Whitelisted proxy server has the responsibility to check the user's IP address via the Eden API.

### Revised Minecraft Account Linking Flow (`POST /minecraft/link`)

*This section assumes that an unidentified player has a Discord account, and joined the community's Discord guild.*

1. For Java Edition players, the unidentified player requires to click a link where it performs
   Discord OAuth2 process. Once the OAuth2 process is completed, the player is automatically linked
   to their Discord account. No additional input required.

2. Clickable links are not supported in Bedrock. To solve this issue, unidentified Bedrock players
   have to memorize random colors in all lowercase (scope: colors of the rainbow) separated by
   spaces. (e.g. `red orange blue green`)

3. Frictionless alternative for Bedrock players, Bedrock players can choose `Web Link` where Eden
   generates a short, easy-to-type URL (e.g. `eden.memothelemo.xyz/link/A8k9`). Opening this link
   in any browser should redirect to Discord OAuth2, linking their account automatically upon
   authorization.

   **The linking session code must be within the custom alphabet designed to remove ambiguity of
   similar looking characters**:
   ```
   23456789ABCDEFGHJKMNPQRSTVWXYZabcdefghjklmnpqrstvwxyz
   ```

4. To keep it consistent for all versions of Eden, multiple accounts (Java/Bedrock) in one Discord
   account is still allowed, unless disabled by a server administrator (either multiple account
   linking or global account linking option).

5. Every challenge will be timed out if there's no activity within 10 minutes.

6. If the associated token found in `Authorization` header is not from the EdenMC server, the
   member requires to login to community's Minecraft server.
