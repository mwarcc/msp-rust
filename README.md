# msp-rust

> A surgical, async, strongly-typed Rust client for **MovieStarPlanet 2** — engineered from the transport layer up for **browser-grade fidelity**, deterministic session portability, and real-time presence.

[![Rust](https://img.shields.io/badge/Rust-async-orange?logo=rust)](https://www.rust-lang.org/)
[![Tokio](https://img.shields.io/badge/runtime-Tokio-2c3e50)](https://tokio.rs/)
[![TLS](https://img.shields.io/badge/TLS-BoringSSL%20emulation-4b8bbe)](#the-fingerprinting-problem-ja3-ja4-ja4h-and-http2)
[![API](https://img.shields.io/badge/API-unofficial-yellow)](#disclaimer)

`msp-rust` is **not** a thin JSON wrapper. It is a fully configurable client stack whose design goal is a single word: **coherence**. Every layer — TLS ClientHello, HTTP/2 SETTINGS, header ordering, cookies, locale, timing, and identity — is aligned so that a session looks and behaves like one continuous browser instance, from the first byte of the handshake to the last WebSocket frame.

---

## Table of contents

- [Philosophy: why "surgical"?](#philosophy-why-surgical)
- [The fingerprinting problem: JA3, JA4, JA4H, and HTTP/2](#the-fingerprinting-problem-ja3-ja4-ja4h-and-http2)
- [How msp-rust achieves coherence](#how-msp-rust-achieves-coherence)
- [Browser & platform emulation](#browser--platform-emulation)
- [Locale coherence](#locale-coherence)
- [Header hygiene and JA4H](#header-hygiene-and-ja4h)
- [Proxies and `enforce_proxy`](#proxies-and-enforce_proxy)
- [Sessions, cookies, and portable state](#sessions-cookies-and-portable-state)
- [Behavioral stealth: humanized pacing](#behavioral-stealth-humanized-pacing)
- [Real-time events](#real-time-events)
- [Architecture](#architecture)
- [Configuration reference](#configuration-reference)
- [Endpoint catalogue](#endpoint-catalogue)
- [Error model](#error-model)
- [Security recommendations](#security-recommendations)
- [Disclaimer](#disclaimer)
- [Code examples](#code-examples)

---

## Philosophy: why "surgical"?

Most API wrappers stop at "send valid JSON, parse valid JSON". That is sufficient only against services that trust their clients. MovieStarPlanet 2, like most modern platforms, sits behind an inspection layer that evaluates **how** a request is made, not just **what** it contains.

A generic Rust HTTP client is trivially distinguishable from a browser at three independent levels:

1. **Transport (TLS)** — the ClientHello of `rustls`/`native-tls` differs from Chrome/Firefox in cipher order, extensions, curves, and ALPN.
2. **Protocol (HTTP/2)** — the frame order, window sizes, and header field ordering of a Rust `h2` stack are not those of Blink or Gecko.
3. **Behavior** — mechanical timing, missing cookies across restarts, inconsistent `Accept-Language`, or a User-Agent that claims Chrome 128 while the TLS says "Rust library".

`msp-rust` treats these as a single, indivisible surface. The word **surgical** means: nothing is left to a generic default. Every knob that contributes to the observable signature is either driven by a real browser profile or exposed as an explicit, coherent configuration option — and the library actively refuses to let you create an incoherent combination.

---

## The fingerprinting problem: JA3, JA4, JA4H, and HTTP/2

To understand what the client protects, you need to understand what a server can measure.

### JA3 — the classic TLS fingerprint

**JA3** is a hash computed from selected fields of the TLS **ClientHello**: the TLS version, the ordered list of cipher suites, the ordered list of extensions, the elliptic curves, and the curve formats. Because different TLS libraries emit these in different orders and with different values, JA3 is a strong "which library sent this?" signal.

A vanilla Rust client produces a **stable, well-known JA3** that maps directly to "rustls" or "openssl" — an immediate red flag when the request headers claim to be Chrome.

### GREASE and extension permutation — why a *fixed* JA3 is now itself suspicious

Since Chrome 110+, browsers deliberately **randomize the order of TLS extensions on every connection** and inject **GREASE** (Generate Random Extensions And Sustain Extensibility) values. The consequence is subtle but critical: a *real* Chrome does **not** have one JA3 — it has a family of JA3s that shuffle per connection.

This means a naive impersonator that hard-codes one "Chrome JA3" is now paradoxically *easier* to detect, because it never shuffles. Correct emulation requires replicating **the shuffling algorithm itself**, not a frozen snapshot of its output.

### JA4 — the modern successor

**JA4** is the evolution of JA3. It is more structured (it sorts extensions to stay stable under permutation, encodes ALPN and TLS version explicitly, and is human-readable). JA4 was designed specifically to survive Chrome's extension shuffling while still separating browsers from tools.

### JA4H — the HTTP layer fingerprint

**JA4H** extends fingerprinting *above* TLS, into the request itself. It hashes:

- the HTTP **method**,
- the HTTP **version**,
- the **presence and order of HTTP headers**,
- the presence of cookies and `Accept-Language`,

…so that even a perfect TLS impersonation fails if the HTTP header set or ordering doesn't match the claimed browser. This is why simply "setting a Chrome User-Agent" is worthless: JA4H sees the *shape* of your header block.

### HTTP/2 fingerprinting (Akamai-style)

At the HTTP/2 layer, servers inspect low-level protocol control data that has nothing to do with your application:

- the **order and values of the `SETTINGS` frame** (`HEADER_TABLE_SIZE`, `ENABLE_PUSH`, `INITIAL_WINDOW_SIZE`, `MAX_HEADER_LIST_SIZE`),
- the **`WINDOW_UPDATE`** delta sent immediately after connection (Chrome sends a large ~15 MB increment),
- the **pseudo-header order** (`:method`, `:authority`, `:scheme`, `:path`),
- stream priority and dependency information.

A standard Rust `h2` client emits defaults that do not match Blink. If your TLS says "Chrome 137" but your HTTP/2 window sizes are Rust defaults, the server sees a **mismatch** and flags the connection.

### The core insight

> A fingerprint is only useful if it is **coherent** across all layers. The weakest, most-forgotten layer is what betrays you. Detection is a search for **contradiction**: Chrome TLS + Rust HTTP/2 + missing cookies + `en-US` header on a French account + millisecond-perfect request timing = bot.

`msp-rust` is built to eliminate every one of those contradictions.

---

## How msp-rust achieves coherence

`msp-rust` is built on [`wreq`](https://github.com/0x676e67/wreq) and [`wreq-util`](https://lib.rs/crates/wreq-util), which use **BoringSSL** (the same TLS engine as Chromium) to reproduce browser handshakes byte-for-byte, and expose fine-grained control over the HTTP/2 stack.

| Layer | Signal | How it is handled |
| --- | --- | --- |
| TLS | JA3 / JA4, cipher & extension order | Driven by the selected `Emulation` profile via BoringSSL |
| TLS | Extension permutation + GREASE | Reproduced **per connection** with the same algorithm as the target Chrome version |
| HTTP/2 | `SETTINGS` order & values | Bundled per-profile in `wreq-util` |
| HTTP/2 | `WINDOW_UPDATE`, `INITIAL_WINDOW_SIZE` | Matched to the emulated browser version |
| HTTP/2 | Pseudo-header order | Enforced via the profile's `PseudoOrder` |
| HTTP | Default header set & order (JA4H) | Owned by the emulation profile — **the app never overrides them** |
| HTTP | `Accept-Language` | Coherent with the account's region |
| Session | Cookies | Persisted and restored across runs |
| Behavior | Request timing | Optional humanized pacing |
| Identity | Device ID, profile/platform | Stable and persisted in exported state |

The client's job is twofold: **(1)** let the emulation own everything fingerprint-relevant, and **(2)** never introduce an application-level contradiction.

---

## Browser & platform emulation

Choose a browser family and operating system. The selection determines the entire TLS + HTTP/2 + default-header signature.

```rust
use msp_rust::MspClient;
use wreq_util::{Platform, Profile};

let client = MspClient::builder()
    .profile(Profile::Chrome137)   // modern, current fingerprint
    .platform(Platform::Windows)
    .build()?;
```

### Available profiles

The builder is wired for current, credible versions:

- **Chrome**: 118, 124, 128, 131, 133, 135, 136, 137
- **Firefox**: 117, 128, 133, 136, 139
- **Platforms**: Windows, macOS, Linux

> **Why version currency matters.** A profile for Chrome 128 in 2026 is a signal in itself — almost no real user runs a browser that old, and the extension-permutation algorithm is version-specific. `msp-rust` defaults to a **current** profile (`Chrome137`) and its randomization pools only draw from recent versions.

### Randomized selection (multi-account de-correlation)

```rust
use msp_rust::{BrowserBrand, MspClient};

let client = MspClient::builder()
    .random_profile(BrowserBrand::Chrome)  // picks from Chrome 133/135/136/137
    .random_platform()                     // picks from Windows/macOS/Linux
    .build()?;
```

Randomization happens **once, at build time**, and then stays fixed for the whole session — because a browser does not change its identity mid-session. The chosen values are queryable via `client.profile()` and `client.platform()` and are persisted in exported state.

### Platform coherence guard

The builder validates that the browser/OS pair is credible. For example, a Safari desktop profile is automatically pinned to macOS, because "Safari on Windows" is a contradiction no real user produces. This guard runs both at fresh build and at state restore.

---

## Locale coherence

`Accept-Language` is part of JA4H and is one of the most common contradiction sources: an account registered in the French region sending `en-US` headers is inconsistent.

`msp-rust` derives a credible `Accept-Language` from the account's **region**, and applies it **once, globally**, at the client level — so it replaces the profile's value in place without disturbing header ordering.

```rust
// Explicit override when you need a specific locale:
let client = MspClient::builder()
    .locale("en-US,en;q=0.9")
    .build()?;
```

Resolution order: **explicit `.locale()`** → **derived from the restored region** → **French default**. Regions mapped out of the box include US, GB, FR, DE, ES, IT, NL, DK, SE, NO, FI, PL, and TR.

---

## Header hygiene and JA4H

This is where most impersonation libraries quietly fail. The emulation profile already emits a complete, correctly-ordered browser header block (`user-agent`, `sec-ch-ua*`, `accept`, `accept-encoding`, `accept-language`, `sec-fetch-*`). If the application re-inserts those headers per request, it **overrides their values and disturbs their order**, breaking JA4H even though TLS is perfect.

`msp-rust` follows a strict rule: **the application only sets headers a browser genuinely adds per request** — `content-type`, `origin` (on cross-site POSTs), `referer`, and `authorization`. Everything fingerprint-relevant is left to the emulation layer. `accept-encoding` in particular is never set by hand, because `wreq` negotiates the exact encoding the emulated browser would, and decodes it transparently.

The transport is also hardened to match a browser: `TCP_NODELAY` enabled, HTTPS-only, and automatic `Referer` injection on redirects disabled (the client manages `Referer` explicitly).

---

## Proxies and `enforce_proxy`

Network-level identity is as important as TLS. IP reputation, geolocation consistency with the account region, and the ability to isolate accounts across egress addresses all depend on proxy support.

`msp-rust` supports:

- **Schemes**: `http`, `https`, `socks5`, `socks5h`.
- **Authenticated proxies**: `http://user:pass@host:port`.
- **Compact form**: `host:port:user:pass` — automatically normalized to `scheme://user:pass@host:port`. This is the format most proxy vendors export, so you can paste it directly.

```rust
let client = MspClient::builder()
    .proxy("gate.provider.com:8080:user123:secretpass")
    .enforce_proxy(true)
    .build()?;
```

### Why `enforce_proxy` exists

`enforce_proxy(true)` is a **safety interlock**, not a convenience. When enabled, the client **refuses to build** (returning `MspError::InvalidProxy`) if no valid proxy is configured. This guarantees that a misconfiguration, a dropped environment variable, or a typo can **never** silently fall back to your real IP address.

For any workflow where a direct connection must *never* happen — multi-account operation, region-locked testing, or simply protecting your origin — `enforce_proxy` turns "I hope the proxy is set" into "the program cannot run without it". This constraint is also persisted in exported state, so a restored session inherits the same guarantee.

---

## Sessions, cookies, and portable state

A session in `msp-rust` is a single, shared, async-safe object holding the access token, refresh token, profile identity, region, device ID, and token expiry — all managed together so that every endpoint sees a consistent view.

### Full session portability

`export_state()` serializes the **complete** session to a schema-versioned structure, and `from_state()` reconstructs an identical client later:

- access & refresh tokens, profile ID, sub ID, region, device ID, expiry;
- browser profile, platform, proxy configuration, and `enforce_proxy` flag;
- **cookies** (see below);
- a `schema_version` that is checked on restore for forward-compatibility.

This means a restored client is **byte-for-byte the same identity** it was before: same fingerprint, same device, same egress policy, same cookies. There is no "new browser" discontinuity between runs.

### Cookie persistence — a real browser keeps its jar

A browser does not throw away its cookies when you close the tab, and neither should an impersonator. `msp-rust` uses a **persistent cookie jar** that:

- wraps `wreq`'s native cookie store (so domain/path/expiry/HTTP-version behavior stays RFC-correct),
- transparently captures every `Set-Cookie` into a serializable structure (`name`, `value`, `domain`, `path`, `secure`, `http_only`, `expires_at`),
- is exported inside `SessionState` and **replayed** into the jar on restore.

The result is genuine **cross-run session continuity**: cookies set by the server in one run are present and sent in the next, exactly as a returning browser would.

> Exported sessions contain access tokens, refresh tokens, and cookies. Treat the file like a password: never commit it, never log it, never share it.

---

## Behavioral stealth: humanized pacing

Even a perfect fingerprint is defeated by mechanical behavior. A bot that fires requests at millisecond-perfect intervals is trivially separable from a human by timing analysis alone.

`msp-rust` includes an optional **pacer** that injects a randomized delay between actions:

```rust
use std::time::Duration;

let client = MspClient::builder()
    .stealth(Duration::from_millis(150), Duration::from_millis(800))
    .build()?;

// In your action loop:
client.pace().await;   // random 150–800 ms; no-op if stealth is disabled
```

`pace()` is always safe to leave in your code: when stealth is not enabled it is an instant no-op. The random duration is drawn *before* the await point, so it composes cleanly with `Send` futures and Tokio's scheduler.

---

## Real-time events

Authentication starts a presence WebSocket task backed by a Tokio broadcast bus. Subscribe before or after login and consume strongly-typed events: presence pings, relationship changes, passive rewards, sent messages, and an `Unknown` fallback for forward-compatibility. The bus is bounded; slow consumers must handle `RecvError::Lagged` per their needs.

---

## Architecture

```text
MspClient
├── wreq HTTP client      (BoringSSL TLS emulation + HTTP/2 shaping + cookies + proxy)
├── PersistentJar         (serializable cookie store, exported/restored with state)
├── SessionStore          (Arc<RwLock<Option<MspSession>>>)
├── StealthConfig         (humanized pacing)
├── EventBus              (Tokio broadcast channel)
└── Endpoints (borrow the client → shared config, session & identity)
    ├── auth
    ├── profiles
    ├── messaging
    ├── ugcs / comments
    ├── greetings / collects
    ├── attributes
    └── reservations
```

Endpoint accessors borrow the main client, so networking configuration, session state, cookie jar, and device identity remain consistent across every operation.

---

## Configuration reference

| Builder method | Purpose |
| --- | --- |
| `.profile(Profile)` | Fix the browser TLS/HTTP2/header signature |
| `.platform(Platform)` | Fix the operating system dimension of the fingerprint |
| `.random_profile(BrowserBrand)` | Randomize the profile within a brand (Chrome / Firefox / Any) |
| `.random_platform()` | Randomize the OS at build time |
| `.locale(impl Into<String>)` | Override `Accept-Language` explicitly |
| `.device_id(impl Into<String>)` | Pin a device ID (otherwise a stable UUID is generated) |
| `.proxy(impl Into<String>)` | Set an HTTP/HTTPS/SOCKS5 proxy (full or compact form) |
| `.enforce_proxy(bool)` | Refuse to build without a valid proxy |
| `.timeout(Duration)` | Overall request timeout |
| `.connect_timeout(Duration)` | Connection establishment timeout |
| `.stealth(min, max)` | Enable humanized request pacing |
| `.from_state(SessionState)` | Rebuild an identical client from exported state |

---

## Endpoint catalogue

| Area | Capabilities |
| --- | --- |
| Authentication | Password grant, profile resolution, refresh grant, JWT claim parsing, silent refresh |
| Sessions | Shared async store, expiry checks, JSON export/restore, schema versioning, cookie persistence |
| Browser emulation | Chrome & Firefox profiles, Windows/macOS/Linux, randomized & coherence-checked selection |
| Proxies | HTTP, HTTPS, SOCKS5/SOCKS5H, authenticated proxy normalization, enforced proxy mode |
| Profiles | Prefix search, game filtering, identity lookup |
| Messaging | Find/create conversations, send messages, list conversations, unread IDs, history, mark read |
| UGC | Fetch UGC metadata, comment count, decode BSON-backed status text from the CDN |
| Comments | Post comments to UGC threads |
| Greetings | Read greeting definitions and send greetings |
| Rewards | Claim daily pickups, VIP pickups, inspect and claim profile collects |
| Attributes | Read profile attributes and update individual `additionalData` keys |
| Reservations | Reserve chatroom and quiz instances and derive Socket.IO URLs |
| Events | Ping responses, relationship changes, passive rewards, sent messages, unknown event fallback |

---

## Error model

All public operations return `msp_rust::Result<T>`. Errors are grouped into actionable variants:

- `Authentication` — authentication-specific failures
- `NoSession` — an authenticated endpoint was called without a session
- `Network` — HTTP transport failures from `wreq`
- `Json` — serialization or response decoding failures
- `Jwt` — malformed tokens or missing claims
- `Api { status, body }` — upstream protocol or HTTP errors
- `Base64` / `UrlEncoded` — payload conversion failures
- `InvalidProxy` — invalid or missing enforced proxy configuration

---

## Security recommendations

- Load credentials from environment variables or a secret manager — never hard-code them.
- Never store exported sessions in source control.
- Never print access tokens, refresh tokens, passwords, or authenticated proxy URLs.
- Keep the browser profile and platform **coherent** for the entire session.
- Use `enforce_proxy(true)` whenever a direct connection must never occur.
- Respect service limits; combine `.stealth(..)` with sane request volumes.

---



## Disclaimer

This is an independent, unofficial project and is **not** affiliated with, endorsed by, or sponsored by MovieStarPlanet. Upstream endpoints can change without notice. You are responsible for complying with applicable terms, laws, account rules, and rate limits. Use the library only with accounts and data you are authorized to access. Fingerprint fidelity is a technical property, **not** a guarantee of anonymity: IP reputation, request behavior, and application-level consistency all matter.

---

## Code examples

> Each example assumes an authenticated `client` unless a login call is shown.

### Quick start

```rust
use msp_rust::MspClient;

#[tokio::main]
async fn main() -> msp_rust::Result<()> {
    let client = MspClient::builder().build()?;
    let session = client.auth().login("username", "password", "FR").await?;
    println!("Authenticated profile: {}", session.profile_id);
    Ok(())
}
```

### Fixed profile, platform, and locale

```rust
use msp_rust::MspClient;
use wreq_util::{Platform, Profile};

let client = MspClient::builder()
    .profile(Profile::Firefox139)
    .platform(Platform::Linux)
    .locale("en-US,en;q=0.9")
    .build()?;
```

### Randomized identity + stealth pacing (multi-account)

```rust
use std::time::Duration;
use msp_rust::{BrowserBrand, MspClient};

let client = MspClient::builder()
    .random_profile(BrowserBrand::Any)
    .random_platform()
    .stealth(Duration::from_millis(150), Duration::from_millis(800))
    .build()?;

println!("Identity: {:?} / {:?}", client.profile(), client.platform());
```

### Proxy with enforcement

```rust
use msp_rust::MspClient;

let client = MspClient::builder()
    .proxy("gate.provider.com:8080:user123:secretpass")
    .enforce_proxy(true)   // build() fails if the proxy is missing/invalid
    .build()?;
```

### Export a session (tokens + cookies)

```rust
let state = client.export_state().await.expect("active session");
std::fs::write("session.json", state.to_json()?)?;
```

### Restore a session — no re-login, same identity and cookies

```rust
use msp_rust::{MspClient, SessionState};

let json  = std::fs::read_to_string("session.json")?;
let state = SessionState::from_json(&json)?;

if state.is_access_token_expired() {
    eprintln!("Exported access token expired; refresh after restoring.");
}

let client = MspClient::builder()
    .from_state(state)   // profile, platform, proxy, locale, cookies all restored
    .build()?;
```

### Real-time events

```rust
use msp_rust::{MspClient, MspEvent};

let client = MspClient::builder().build()?;
let mut events = client.events();

client.auth().login("username", "password", "FR").await?;

while let Ok(event) = events.recv().await {
    match event {
        MspEvent::MessageSent(m)         => println!("{}: {}", m.author, m.message_body),
        MspEvent::PassiveRewardEarned(r) => println!("Earned {} XP", r.xp),
        MspEvent::Unknown { message_type, payload } =>
            println!("Unknown event {message_type}: {payload}"),
        _ => {}
    }
}
```

### Profile search and identity

```rust
let results = client.profiles().search_profiles("name", "FR", 1, 20, Some("j68d")).await?;
for node in results.nodes {
    if let Some(identity) = client.profiles().get_profile_identity(&node.id).await? {
        println!("{} ({})", identity.name, identity.id);
    }
}
```

### Messaging

```rust
let conversation = client.messaging().get_or_create_conversation("other-profile-id").await?;
let receipt = client.messaging()
    .send_message(&conversation.conversation_id, "Hello from Rust!")
    .await?;
println!("Sent at {}", receipt.timestamp);

let history = client.messaging().get_chat_history(&conversation.conversation_id, 50).await?;
client.messaging().mark_conversation_as_read(&conversation.conversation_id).await?;
```

### Profile attributes

```rust
let attributes = client.attributes().get(None).await?;
println!("Avatar: {}", attributes.avatar_id);

let updated = client.attributes()
    .update_additional_data_key("exampleKey", serde_json::json!(true))
    .await?;
println!("Updated: {}", updated.additional_data);
```

### UGC and comments

```rust
if let Some(ugc) = client.ugcs().get_ugc_by_id("ugc-id").await? {
    println!("UGC type: {}", ugc.ugc_type);
    println!("Comments: {}", client.ugcs().get_comments_count(&ugc.id).await?);
    if let Some(status) = client.ugcs().get_status_text(&ugc.id).await? {
        println!("Status: {status}");
    }
    let comment = client.comments().post(&ugc.id, "Great post!").await?;
    println!("Created comment {}", comment.comment_id);
}
```

### Greetings

```rust
let definitions = client.greetings().get_greeting_definitions().await?;
if let Some(greeting) = definitions.first() {
    let result = client.greetings()
        .send_greeting(&greeting.greeting_type, "receiver-profile-id")
        .await?;
    println!("Greeting sent: {}", result.success);
}
```

### Daily rewards and collects

```rust
client.collects().collect_pickup().await?;
let available = client.collects().get_collects().await?;
let collect_types: Vec<&str> = available.iter().map(|c| c.collect_type.as_str()).collect();
if !collect_types.is_empty() {
    let claimed = client.collects().claim_collects(&collect_types).await?;
    println!("Claimed {} collect groups", claimed.len());
}
```

### Room reservations

```rust
let room = client.reservations().chatroom("level-id", "version").await?;
println!("Room ID: {} — Socket: {}", room.room_id, room.socket_url);

let quiz = client.reservations().quiz().await?;
println!("Quiz room: {}", quiz.room_id);
```

### Error handling

```rust
use msp_rust::{MspClient, MspError};

match client.profiles().get_profile_identity("profile-id").await {
    Ok(Some(profile)) => println!("Found {}", profile.name),
    Ok(None)          => println!("Profile not found"),
    Err(MspError::NoSession) => eprintln!("Authenticate first"),
    Err(MspError::Api { status, body }) => eprintln!("Upstream API error ({status}): {body}"),
    Err(error) => eprintln!("Request failed: {error}"),
}
```
