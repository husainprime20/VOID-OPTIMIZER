# Access — Discord OAuth2 + PKCE

VOID has no server. There is no login API of ours to call, no session table, no
place a token could be checked by somebody else. So the gate is built the only
way it can be built honestly: the browser talks to Discord, Discord redirects
back to a loopback port on the machine that is running VOID, and VOID asks
Discord directly whether the person holds the role — every launch, and again on
demand.

The pattern comes from the BSI Customizer hand-off we adopted as a design
checklist. The code here is VOID's own; none of it was copied.

---

## What is enforced

| Question | Answer |
| --- | --- |
| Public client, no secret? | Yes. `client_secret` never appears in the source, the config, or the binary. |
| PKCE? | S256, 32 random bytes from `BCryptGenRandom`, verifier never leaves the process. |
| `state`? | 16 random bytes, compared before the code is exchanged; mismatch is a hard refusal. |
| Redirect | `http://127.0.0.1:43713/auth/discord/callback`, one request, then the port closes. |
| Role check | `GET /users/@me/guilds/{guild}/member` at sign-in **and** at every start. A 403/404 is a closed door. |
| Cached authorisation? | Never. The saved tier is a label for the UI; the gate re-derives it or stays shut. |
| Where is the refresh token? | `%APPDATA%\VOID\auth.token`, sealed with DPAPI — bound to this Windows account on this machine. |
| Sign-out? | `auth.session` and `auth.token` are deleted, and the gate closes in the same press. |
| On any error? | Closed. Network failure, parse failure, timeout, user cancel — all of them. |

The engine is the authority, not the window: `engine::apply_ids` refuses when the
gate is shut, so a profile, a palette action, match mode and auto-switch are all
blocked by one check in one place. `void-cli` deliberately stays open — it is the
escape hatch that makes a broken gate recoverable.

---

## Shipping a gated build

1. Create an application at <https://discord.com/developers/applications>. Copy the
   **public client id** (not the secret).
2. In **OAuth2 → Redirects** add exactly
   `http://127.0.0.1:43713/auth/discord/callback`.
3. Enable the **`guilds.members.read`** scope on the app (OAuth2 → General → Scopes
   is implied by the request, but the toggle must be allowed for the app).
4. Turn on **Developer Mode** in Discord, right-click your server → *Copy Server ID*,
   right-click each role → *Copy Role ID*.
5. Copy `auth.example.json` to `auth.json` next to `VOID.exe` (or into
   `%APPDATA%\VOID\auth.json`) and fill it in. `VOID_AUTH_JSON=<path>` overrides both,
   which is what CI and a portable build use.

No `auth.json` means no gate: the build behaves exactly as it always has. That is
on purpose — the gate is a feature you opt into by shipping a policy.

### Fields

| Key | Meaning |
| --- | --- |
| `client_id` | Public application id. Empty ⇒ no gate. |
| `redirect_port` | Loopback port. Default `43713`. |
| `guild_id` | The server the role is read from. Empty ⇒ allowlist mode. |
| `required_tier` | Minimum tier: `user` < `premium` < `dev` < `owner`. Highest role held wins. |
| `roles` | Role id → `{ "tier", "label" }`. Any one of these opens the gate. |
| `allow_users` | User ids. Only consulted when there is **no** role gate — a member must hold the role no matter who they are. |
| `note` | Shown on the lock screen instead of the raw guild id. |
| `invite_url` | Adds an *open the server* button to the lock screen. |

---

## The honest limit

A local check stops a build being passed around. It does not stop a determined
person on their own machine, who can patch the binary, edit `auth.json`, or call
`void-cli apply`. Anything that runs entirely on the user's PC can be subverted by
that user; pretending otherwise would be theatre. If enforcement ever has to be
real, it needs a server that holds the tokens — and VOID does not have one, by
design.

Two smaller truths worth writing down:

* The Discord **presence** link in Settings is a different thing. It is cosmetic —
  it shows your tag and score in the Discord client — and it is not the gate.
* The saved tier label is a convenience for the UI. It is never treated as proof;
  the gate re-checks before it opens.

---

## Testing it

`VOID.exe --selftest` runs the parts that need no account:

```
sha256_abc=match              FIPS 180-4 vector
pkce_s256=match               RFC 7636 appendix B vector
policy_role_gate=closed/closed/open
policy_unconfigured=denied
callback_parse=match
gate=open|closed
policy=configured|none
```

`cargo test` carries the same checks as unit tests. Everything else — the browser
round trip, the loopback listener, DPAPI — needs Windows and a real account, and
that is the part no CI run can honestly claim to have verified.
