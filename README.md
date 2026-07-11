# msp-rust

> An async, strongly typed Rust client for MovieStarPlanet 2 services, built for browser-grade HTTP behavior, structured sessions, and real-time events.

[![Rust](https://img.shields.io/badge/Rust-async-orange?logo=rust)](https://www.rust-lang.org/)
[![Tokio](https://img.shields.io/badge/runtime-Tokio-2c3e50)](https://tokio.rs/)
[![API](https://img.shields.io/badge/API-unofficial-yellow)](#disclaimer)

`msp-rust` provides one cohesive client for authentication, profiles, messaging, comments, greetings, rewards, UGC, room reservations, profile attributes, session persistence, and live presence events.

## Why msp-rust?

Most API wrappers stop at sending JSON. `msp-rust` is designed around the behavior expected by the upstream services:

- **Browser-grade networking** — powered by `wreq` and `wreq-util` with selectable browser and platform emulation.
- **TLS and JA3-aware profiles** — choose Chrome or Firefox profiles instead of relying on a generic Rust TLS fingerprint.
- **One shared session** — access and refresh tokens, profile identity, region, device ID, and expiration are managed together.
- **Strongly typed results** — common API responses are exposed as Rust models rather than unstructured JSON.
- **Real-time events** — a Tokio broadcast event bus surfaces presence, relationship, reward, and message events.
- **Proxy controls** — supports HTTP, HTTPS, SOCKS5, authenticated proxies, and optional proxy enforcement.
- **Portable state** — export a session to JSON and restore it later with schema compatibility checks.
- **Async by default** — all network operations integrate naturally with Tokio applications.

## TLS, JA3, and browser emulation

Many services evaluate more than request headers. A connection can also expose its TLS ClientHello, cipher ordering, extensions, ALPN negotiation, HTTP/2 settings, and other transport characteristics. JA3 is a compact fingerprint derived from parts of the TLS ClientHello and is one signal among many.

`msp-rust` uses `wreq` plus `wreq-util` emulation profiles to make the HTTP stack behave more like a selected browser family:

```rust
use msp_rust::MspClient;
use wreq_util::{Platform, Profile};

let client = MspClient::builder()
    .profile(Profile::Chrome128)
    .platform(Platform::Windows)
    .build()?;
```

Available profiles in the current builder include Chrome 118, Chrome 124, Chrome 128, Firefox 117, and Firefox 128, paired with Windows, macOS, or Linux.

> **Important:** JA3 similarity is not anonymity and is not a guarantee that traffic is indistinguishable from a real browser. IP reputation, request behavior, cookies, HTTP/2 details, timing, and application-level consistency can all matter. Use a browser profile and platform combination that remains coherent throughout a session.

## Features

| Area | Capabilities |
| --- | --- |
| Authentication | Password grant, profile resolution, refresh grant, JWT claim parsing, silent refresh |
| Sessions | Shared async store, expiry checks, JSON export and restore, schema versioning |
| Browser emulation | Chrome and Firefox profiles, Windows/macOS/Linux platforms, randomized selection |
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

## Architecture

```text
MspClient
├── wreq HTTP client (browser emulation + cookies + proxy)
├── SessionStore (Arc<RwLock<Option<MspSession>>>)
├── EventBus (Tokio broadcast channel)
└── Endpoints
    ├── auth
    ├── profiles
    ├── messaging
    ├── ugcs / comments
    ├── greetings / collects
    ├── attributes
    └── reservations
```

The endpoint accessors borrow the main client, so networking configuration, session state, and device identity remain consistent across operations.

## Quick start

```rust
use msp_rust::MspClient;

#[tokio::main]
async fn main() -> msp_rust::Result<()> {
    let client = MspClient::builder().build()?;

    let session = client
        .auth()
        .login("username", "password", "FR")
        .await?;

    println!("Authenticated profile: {}", session.profile_id);
    Ok(())
}
```

## Client configuration

### Fixed browser profile and platform

```rust
use msp_rust::MspClient;
use wreq_util::{Platform, Profile};

let client = MspClient::builder()
    .profile(Profile::Firefox128)
    .platform(Platform::Linux)
    .build()?;
```

### Randomized profile selection

```rust
use msp_rust::{BrowserBrand, MspClient};

let client = MspClient::builder()
    .random_profile(BrowserBrand::Chrome)
    .random_platform()
    .build()?;
```

Randomization occurs when the client is built. The chosen profile and platform remain available through `client.profile()` and `client.platform()`.

### Proxy support

```rust
use msp_rust::MspClient;

let client = MspClient::builder()
    .proxy("http://user:password@127.0.0.1:8080")
    .enforce_proxy(true)
    .build()?;
```

The builder also accepts compact `host:port:user:password` proxy strings and normalizes them to `http://user:password@host:port`. When `enforce_proxy(true)` is enabled, building without a proxy returns `MspError::InvalidProxy`.

## Session persistence

Export an authenticated session:

```rust
let state = client.export_state().await.expect("active session");
std::fs::write("session.json", state.to_json()?)?;
```

Restore it later:

```rust
use msp_rust::{MspClient, SessionState};

let json = std::fs::read_to_string("session.json")?;
let state = SessionState::from_json(&json)?;

if state.is_access_token_expired() {
    eprintln!("The exported access token is expired; refresh after restoring.");
}

let client = MspClient::builder()
    .from_state(state)
    .build()?;
```

Session exports contain access and refresh tokens. Treat them like passwords: never commit them, log them, or share them.

## Real-time events

Authentication starts the presence WebSocket task. Subscribe before or after login and process typed events from the broadcast receiver:

```rust
use msp_rust::{MspClient, MspEvent};

let client = MspClient::builder().build()?;
let mut events = client.events();

client.auth().login("username", "password", "FR").await?;

while let Ok(event) = events.recv().await {
    match event {
        MspEvent::MessageSent(message) => {
            println!("{}: {}", message.author, message.message_body);
        }
        MspEvent::PassiveRewardEarned(reward) => {
            println!("Earned {} XP", reward.xp);
        }
        MspEvent::Unknown { message_type, payload } => {
            println!("Unknown event {message_type}: {payload}");
        }
        _ => {}
    }
}
```

The event bus uses a bounded Tokio broadcast channel. Slow consumers should handle `RecvError::Lagged` according to their application requirements.

## Error handling

All public operations return `msp_rust::Result<T>`. Errors are grouped into actionable variants:

- `Authentication` — authentication-specific failures
- `NoSession` — an authenticated endpoint was called without a session
- `Network` — HTTP transport failures from `wreq`
- `Json` — serialization or response decoding failures
- `Jwt` — malformed tokens or missing claims
- `Api` — upstream protocol or HTTP errors
- `Base64` / `UrlEncoded` — payload conversion failures
- `InvalidProxy` — invalid or missing enforced proxy configuration

```rust
use msp_rust::{MspClient, MspError};

match client.profiles().get_profile_identity("profile-id").await {
    Ok(Some(profile)) => println!("Found {}", profile.name),
    Ok(None) => println!("Profile not found"),
    Err(MspError::NoSession) => eprintln!("Authenticate first"),
    Err(MspError::Api { status, body }) => {
        eprintln!("Upstream API error ({status}): {body}");
    }
    Err(error) => eprintln!("Request failed: {error}"),
}
```

## Security recommendations

- Load account credentials from environment variables or a secret manager.
- Never store exported sessions in source control.
- Avoid printing access tokens, refresh tokens, passwords, or authenticated proxy URLs.
- Use timeouts appropriate for your workload.
- Use proxy enforcement when direct connections must never occur.
- Respect service limits and avoid automated behavior that could affect other users.

## Complete examples

The following focused examples cover the public endpoint groups. Each assumes an authenticated `client` unless login is shown.

### Profile search and identity

```rust
let results = client
    .profiles()
    .search_profiles("name", "FR", 1, 20, Some("j68d"))
    .await?;

for node in results.nodes {
    if let Some(identity) = client
        .profiles()
        .get_profile_identity(&node.id)
        .await?
    {
        println!("{} ({})", identity.name, identity.id);
    }
}
```

### Messaging

```rust
let conversation = client
    .messaging()
    .get_or_create_conversation("other-profile-id")
    .await?;

let receipt = client
    .messaging()
    .send_message(&conversation.conversation_id, "Hello from Rust!")
    .await?;

println!("Sent at {}", receipt.timestamp);

let history = client
    .messaging()
    .get_chat_history(&conversation.conversation_id, 50)
    .await?;

client
    .messaging()
    .mark_conversation_as_read(&conversation.conversation_id)
    .await?;
```

### Profile attributes

```rust
let attributes = client.attributes().get(None).await?;
println!("Avatar: {}", attributes.avatar_id);

let updated = client
    .attributes()
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
    let result = client
        .greetings()
        .send_greeting(&greeting.greeting_type, "receiver-profile-id")
        .await?;

    println!("Greeting sent: {}", result.success);
}
```

### Daily rewards and collects

```rust
client.collects().collect_pickup().await?;

let available = client.collects().get_collects().await?;
let collect_types: Vec<&str> = available
    .iter()
    .map(|collect| collect.collect_type.as_str())
    .collect();

if !collect_types.is_empty() {
    let claimed = client.collects().claim_collects(&collect_types).await?;
    println!("Claimed {} collect groups", claimed.len());
}
```

### Room reservations

```rust
let room = client.reservations().chatroom("level-id", "version").await?;
println!("Room ID: {}", room.room_id);
println!("Socket URL: {}", room.socket_url);

let quiz = client.reservations().quiz().await?;
println!("Quiz room: {}", quiz.room_id);
```

## Disclaimer

This is an independent, unofficial project and is not affiliated with, endorsed by, or sponsored by MovieStarPlanet. Upstream endpoints can change without notice. You are responsible for complying with applicable terms, laws, account rules, and rate limits. Use the library only with accounts and data you are authorized to access.
