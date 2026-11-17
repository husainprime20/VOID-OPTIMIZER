# Discord: presence + account link

VOID uses the Discord **application** `1571961942044713070`. Two separate things can be set up.

## 1. Presence art (assets)

1. <https://discord.com/developers/applications> → **VOID** → **Rich Presence → Art Assets**.
2. **Add Image** → upload `docs/branding/VOID-icon-512.png` → name it **`void_idle`**.
3. Add it again → name that copy **`void_matchmode`**.
4. Save, restart Discord, restart VOID.

VOID asks for `void_idle` when idle and `void_matchmode` while a job runs.

## 2. Account authorization (OAuth2)

The link lets VOID read your profile (`identify`) and show your tag in the status bar.

1. Same portal page → **OAuth2** → copy the **Client Secret**.
2. In VOID → **Settings → Discord account → Client secret** → paste → **Save**.
3. Press **Authorize with Discord**. The Discord client opens its own consent dialog —
   press *Authorize*. Nothing is typed into VOID.
4. VOID takes the one-time code, exchanges it **directly with `discord.com` over WinHTTP**,
   and reads `/api/users/@me`. Your name, id and monogram appear in the card.

### Why the secret is yours, not ours

A client secret identifies the *application*. If VOID shipped one inside the binary it would
be extractable by anyone, and the app could impersonate your application against Discord.
So the binary contains no secret: you paste your own into `%APPDATA%\VOID\settings.conf`
(local only, sent nowhere except `discord.com` during the exchange). Everything else —
consent, code, token, profile — is handled by the app.

### What VOID never does

- No token is written to disk; the link lasts for the session.
- No friend list, message, guild or presence data is read — only `identify`.
- No requests go anywhere except `discord.com` and the local Discord pipe.
