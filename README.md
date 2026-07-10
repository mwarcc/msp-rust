# 🪐 msp-rust

> A high-performance, asynchronous Rust client for the **MovieStarPlanet 2** private API — built on `wreq` with full TLS/JA3 fingerprint emulation to look like a real browser.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2021-blue.svg)]()
[![License](https://img.shields.io/badge/license-MIT-green.svg)]()
[![Async](https://img.shields.io/badge/async-tokio-blueviolet.svg)](https://tokio.rs)
[![HTTP](https://img.shields.io/badge/http-wreq-success.svg)](https://github.com/0x676e67/wreq)

`msp-rust` is an unofficial, reverse-engineered API wrapper for MovieStarPlanet 2 written in idiomatic Rust. It handles authentication, session management, GraphQL queries, room reservations, presence WebSockets, greetings, comments, and direct messaging — all with production-grade error handling, structured logging, and fingerprint-resistant transport.

---

## ✨ Features

- 🔐 **Full authentication flow** — password grant, JWT decoding, profile resolution, refresh tokens
- 🌐 **Browser-grade TLS / JA3 fingerprint emulation** via `wreq-util` (Chrome, Firefox profiles)
- 🎭 **Randomized platform & browser profiles** to avoid fingerprinting
- 🧵 **Fully async, thread-safe** — built on Tokio with shared `Arc<RwLock>` session store
- 💬 **Complete messaging API** — conversations, chat history, read receipts, embedded JSON parsing
- 🌟 **Greetings, comments, profile attributes** — full GraphQL support
- 🏟️ **Matchmaker reservations** — chatrooms and StarQuiz, with Engine.IO-aware socket URLs
- 📡 **Presence WebSocket** — automatic Engine.IO / Socket.IO handshake + heartbeat loop
- 🪵 **Structured tracing** — every endpoint instrumented with `tracing` spans
- 🛡️ **Proxy support** — HTTP, HTTPS, SOCKS5
- ⏱️ **Tunable timeouts** — read/write and connect timeouts
- 🧰 **Rich error model** — `MspError` enum with `thiserror` integration

---

## 🌐 TLS, JA3 & Browser Fingerprinting

MSP2 sits behind **AWS CloudFront** (a CDN, not a WAF — there's no public evidence of aggressive JA3/Akamai-style anti-bot challenges). That said, the underlying services still expect traffic that *looks like* a real browser session: a modern TLS handshake, a sensible HTTP/2 preface, and consistent header/cookie behaviour. Sending a raw `hyper` or stock `reqwest` request is technically possible, but it stands out — unusual cipher suites, missing `Sec-CH-UA`, no cookie store, no automatic `User-Agent` rotation, etc.

`msp-rust` therefore uses [`wreq`](https://github.com/0x676e67/wreq) + [`wreq-util`](https://github.com/0x676e67/wreq-util) to emulate the full browser stack. Think of it as **defense-in-depth and traffic consistency**, not as a workaround for an active fingerprinting wall:

| Layer            | What's emulated                                                                                  |
| ---------------- | ------------------------------------------------------------------------------------------------ |
| **TLS**          | Cipher suites, extensions (ALPN, ECH, GREASE, …), supported groups, signature algorithms, curves |
| **HTTP/2**       | Initial window size, header table size, max concurrent streams, frame ordering                   |
| **JA3 / JA4**    | Built-in browser presets match real Chrome / Firefox handshakes                                  |
| **Headers**      | `User-Agent`, `Sec-CH-UA`, `Sec-Fetch-*`, `Accept-Language` are auto-injected per profile        |
| **Cookies**      | Native cookie store enabled by default — `Set-Cookie` from login is automatically replayed      |

### Choosing a profile

```rust
// Fixed profile
let client = MspClient::builder()
    .profile(wreq_util::Profile::Chrome128)
    .platform(wreq_util::Platform::Windows)
    .build()?;
```

### Going random (recommended for automation)

```rust
use msp_client::BrowserBrand;

let client = MspClient::builder()
    .random_profile(BrowserBrand::Chrome)   // picks Chrome124/128 randomly
    .random_platform()                       // Windows / macOS / Linux
    .build()?;
```

Available brands:

- `BrowserBrand::Chrome`  → `Chrome118`, `Chrome124`, `Chrome128`
- `BrowserBrand::Firefox` → `Firefox117`, `Firefox128`
- `BrowserBrand::Any`     → modern mix of Chrome & Firefox

The active profile is exposed at runtime:

```rust
println!("profile  = {:?}", client.profile());   // Profile::Chrome128
println!("platform = {:?}", client.platform());  // Platform::Windows
```

> All profiles in the builder are recent and secure; older Chrome110/Edge builds are intentionally excluded to avoid being fingerprinted as "outdated browser".

---

## 🔌 Proxy Support

```rust
let client = MspClient::builder()
    .proxy("socks5://user:pass@127.0.0.1:9050")
    .build()?;
```

Supports `http://`, `https://`, and `socks5://` schemes.

---

## 📚 API Reference

### `MspClientBuilder`

| Method                            | Description                                                                |
| --------------------------------- | -------------------------------------------------------------------------- |
| `device_id(id)`                   | Set a fixed device ID; otherwise a random uppercase UUIDv4 is generated     |
| `profile(p)`                      | Pin a specific `wreq_util::Profile`                                         |
| `platform(p)`                     | Pin a specific `wreq_util::Platform`                                       |
| `random_platform()`               | Pick OS uniformly at `build()` time                                        |
| `random_profile(brand)`           | Pick browser profile uniformly from a safe list of `BrowserBrand`         |
| `proxy(url)`                      | Configure proxy (`http://`, `https://`, `socks5://`)                        |
| `timeout(Duration)`               | Max read/write duration (default `30s`)                                     |
| `connect_timeout(Duration)`       | TLS handshake timeout (default `10s`)                                      |
| `build()` → `Result<MspClient>`   | Build the client                                                           |

### `MspClient`

| Accessor                  | Returns                |
| ------------------------- | ---------------------- |
| `auth()`                  | `AuthEndpoint`         |
| `attributes()`            | `AttributesEndpoint`   |
| `comments()`              | `CommentsEndpoint`     |
| `greetings()`             | `GreetingsEndpoint`    |
| `messaging()`             | `MessagingEndpoint`    |
| `reservations()`          | `ReservationsEndpoint` |
| `session()`               | `&SessionStore`        |
| `profile()`               | `Profile`              |
| `platform()`              | `Platform`             |

### `AuthEndpoint`

```rust
client.auth().login(username: &str, password: &str, region: &str) -> Result<MspSession>
```

Performs the full OAuth2 password + refresh + profile resolution flow. Spawns a background tokio task that maintains the **presence WebSocket** (Engine.IO v3 with heartbeat every 5s). The bearer is automatically stored in `SessionStore`.

### `AttributesEndpoint`

```rust
// Fetch attributes for any profile (or self if None)
client.attributes().get(profile_id: Option<&str>) -> Result<ProfileAttributes>

// Update a single key in `additionalData` (read-modify-write)
client.attributes().update_additional_data_key("Mood", "noshoes_skating") -> Result<ProfileAttributes>
```

`ProfileAttributes` exposes: `profile_id`, `game_id`, `avatar_id`, `additional_data: serde_json::Value`.

### `CommentsEndpoint`

```rust
// Post a comment to a UGC thread
client.comments().post(thread_id: &str, text: &str) -> Result<SentComment>
```

### `GreetingsEndpoint`

```rust
// Get all greeting definitions (costs, cooldowns, XP formulas, rewards…)
client.greetings().get_greeting_definitions() -> Result<Vec<GreetingDefinition>>

// Send a greeting
client
    .greetings()
    .send_greeting(greeting_type: &str, profile_id: &str)
    -> Result<SendGreetingResult>
```

Supported `greeting_type` values: `Autograph`, `StarGreeting`, `LoveGreeting`, `RainbowGreeting`, `PartyGreeting`, `SuperStarGreeting`.

Failures (cooldown, insufficient funds, etc.) are returned as `MspError::Api { status, body }` with the failure reason and `nextGreetingSecondsRemaining` embedded in the message.

### `MessagingEndpoint`

```rust
// Find an existing 1:1 conversation, or create one
client.messaging().get_or_create_conversation(other_id: &str) -> Result<Conversation>

// Send a message
client.messaging().send_message(conversation_id: &str, body: &str) -> Result<MessageReceipt>

// Paginated list of conversations
client.messaging().get_conversations(page: u32, page_size: u32) -> Result<ConversationPage>

// Conversation history
client.messaging().get_chat_history(conversation_id: &str, page_size: u32) -> Result<Vec<ChatMessage>>

// Mark a conversation as read
client.messaging().mark_conversation_as_read(conversation_id: &str) -> Result<ConversationEntry>
```

`ConversationPage` carries a pre-computed `unread_conversation_ids: Vec<String>` convenience list.

The embedded `latestMessage` JSON-string (which uses PascalCase keys) is auto-parsed into a `LatestMessage` struct and exposed as `latest_message_parsed`.

### `ReservationsEndpoint`

```rust
// Reserve a chatroom slot
client.reservations().chatroom(level: &str, version: &str) -> Result<RoomReservation>

// Reserve a StarQuiz slot
client.reservations().quiz() -> Result<RoomReservation>
```

Returns a `RoomReservation { host_url, room_id, socket_url }` — `socket_url` is pre-formatted with the correct Engine.IO version (3 for chatrooms, 4 for quiz) and path (`/socket.io` vs `/`).

---

## 🧪 Error Handling

All endpoints return `Result<T, MspError>`. Variants:

| Variant            | When                                                            |
| ------------------ | --------------------------------------------------------------- |
| `Authentication`   | Login / token rejection                                        |
| `NoSession`        | An endpoint was called before `login()` or after `clear()`      |
| `Network`          | `wreq` transport error                                          |
| `Json`             | `serde_json` decoding / encoding                                |
| `Jwt`              | JWT layout or `sub` claim missing                               |
| `Api`              | Server returned an error payload (`status`, `body`)             |
| `Base64`           | JWT base64 decoding                                             |
| `UrlEncoded`       | `serde_urlencoded` failure                                      |
| `InvalidProxy`     | Proxy URL couldn't be parsed by `wreq`                          |

Pattern match on them when you need fine-grained control:

```rust
match client.greetings().send_greeting("Autograph", "FRIEND_ID").await {
    Ok(r)  => println!("ok: {}", r.success),
    Err(MspError::Api { status, body }) if status == 200 => {
        eprintln!("rejected by MSP: {body}");
    }
    Err(e) => return Err(e),
}
```

---

## 🪵 Observability

The crate uses [`tracing`](https://docs.rs/tracing). Every public function is instrumented:

```
2024-01-15T10:23:11.123Z  INFO msp_login{username=alice region=EU device_id=…}: msp_client::client::endpoints::auth: MSP Client session successfully initialized profile_id="abc-…"
2024-01-15T10:23:11.456Z DEBUG presence: msp_client::client::endpoints::auth: Presence initialization sequences dispatched.
```

Configure verbosity via `RUST_LOG`:

```bash
RUST_LOG=msp_client=debug,info cargo run
```

---

## ⚙️ Example: build configuration matrix

```rust
// 1) Paranoid — fresh identity every run
let c = MspClient::builder()
    .random_profile(BrowserBrand::Any)
    .random_platform()
    .proxy("socks5://127.0.0.1:9050")
    .build()?;

// 2) Stable — pinned, long-lived
let c = MspClient::builder()
    .profile(wreq_util::Profile::Firefox128)
    .platform(wreq_util::Platform::MacOS)
    .device_id("C93435CE265822D7FA1052789B780D4911F241532AF72F16C81D30BBB0393759")
    .timeout(Duration::from_secs(60))
    .build()?;

// 3) CI / fast
let c = MspClient::builder()
    .profile(wreq_util::Profile::Chrome128)
    .connect_timeout(Duration::from_secs(5))
    .build()?;
```

---

## 🏎️ Performance Notes

`Cargo.toml` ships with optimized profiles for both dev and release:

- **Dev**: `opt-level = 1` for your code, `opt-level = 2` for dependencies, `lld` linker → fast incremental builds with optimized deps.
- **Release**: `opt-level = 3`, `lto = "thin"`, `codegen-units = 16`, `strip = "symbols"`.
- **Production**: `lto = "fat"`, `codegen-units = 1`, `panic = "abort"` — use it for shipped binaries.

The whole client is `Clone`-able: the HTTP engine is internally `Arc`'d, and `SessionStore` is `Arc<RwLock<…>>`. Spawn as many concurrent endpoint handles as you need.

---

## ⚠️ Disclaimer

This project is an **unofficial**, reverse-engineered client for the MovieStarPlanet 2 private API. It is provided for **educational and research purposes only**. Using it may violate the MSP2 Terms of Service and could result in account sanctions. The authors take no responsibility for any consequences arising from its use.

---


