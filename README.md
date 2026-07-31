# `eden-rewrite/redesign`

> An integrated system built specifically for Dystopia (or any Minecraft server community),
  originally to bridge the gap between its Discord guild and Minecraft server.

This branch contains the revised architecture of **Eden System** as its original legacy codebase
was hurriedly implemented for Season 4, and suffered from structural and user experience
limitations. Therefore, this branch surfaced its issues of using Eden from experiences from
players in Dystopia, and its new architecture in **Eden v4** will hopefully last for a long time.

## Issues
1. Players have to **memorize or copy a random 4-word generated code** and message it directly to
   the Eden bot on Discord within a short period of time. This adds friction and confusion for all
   players because they have to read the entire step-by-step process.

2. Eden, since its first version, has no administrative Web UI to handle special account linking
   and manage settings. The leader maintainer of Eden has to do all of the special interventions
   which it will waste time not for the lead maintainer, but for the staff and players.

3. Implementation of Eden v3 was hurriedly developed prior to Season 4 launch, therefore technical
   debt and complexity had been accumulated throughout the codebase.

4. Eden v3 was originally developed to allow Discord guilds outside Dystopia, to add Eden for its
   auto-response features which it adds more complexity of implementing new features.

## New Architecture

In this new architecture, Eden will have both frontend web application (specifically the admin
dashboard), and Rust backend, but the philosophy of bridging gaps between the community's Discord
guild and its Minecraft server still remains.

Unlike previous versions of Eden, the goal of v4 is to ONLY TARGET for one Discord guild, and
not tie any namings back to Dystopia. The community can customize their name to their linking.

### Utilizing Kafka

This replaces the custom database-backed job schedulers with Kafa topics. This new version of Eden
will allow for durable event persistence and fault tolerance.

1. Event Replay
2. Decoupled Asynchronous Processing
3. Multi-consumer fan-out
4. Dead-letter queue & retry policy

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
