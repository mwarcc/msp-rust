# 🪐 msp-rust

> A high-performance, asynchronous Rust client for the **MovieStarPlanet 2** private API — built on `wreq` with full TLS/JA3/JA4 fingerprint emulation and strict Network Identity Isolation to guarantee stealth and account longevity.

[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Edition](https://img.shields.io/badge/edition-2021-blue.svg)]()
[![License](https://img.shields.io/badge/license-MIT-green.svg)]()
[![Async](https://img.shields.io/badge/async-tokio-blueviolet.svg)](https://tokio.rs)
[![HTTP](https://img.shields.io/badge/http-wreq-success.svg)](https://github.com/0x676e67/wreq)

`msp-rust` is an unofficial, reverse-engineered API wrapper for MovieStarPlanet 2 written in idiomatic Rust. It handles authentication, stable session serialization, proxy enforcement, real-time WebSocket events, GraphQL queries, room reservations, comments, and direct messaging — all with production-grade error handling, structured logging, and fingerprint-resistant transport.

---

## ✨ Features

- **Full Authentication Flow** — password grant, JWT decoding, profile resolution, refresh tokens
- **Persistent Session Storage** — export and re-import the complete operational state (`SessionState`) across restarts to bypass password-grant flags
- **Network Identity Isolation** — pin proxies to sessions with strict fail-fast constraints (`enforce_proxy`) preventing host IP leaks
- **Smart Proxy Normalization** — supports native URI formatting and raw provider credentials (`host:port:user:pass`) automatically
- **Asynchronous Event Hooks** — reactive broadcast stream (`tokio::sync::broadcast`) for real-time WebSocket events (messages, requests, rewards)
- **Browser-Grade TLS / JA3 / JA4 Emulation** — dynamic user-agent, HTTP/2 frame settings, and TLS extensions mapping via `wreq-util`
- **Randomized Platform & Browser Profiles** — Chrome and Firefox modern presets to prevent fingerprint tracking
- **Fully Async & Thread-Safe** — built on Tokio with shared `Arc<RwLock>` session store
- **Complete Messaging API** — direct 1:1 conversation initialization, paginated chat history, and unread trackers

---

## 🌐 TLS, JA3 & Browser Fingerprinting

MSP2 sits behind **AWS CloudFront** (a CDN, not a WAF — there's no public evidence of aggressive JA3/Akamai-style anti-bot challenges). That said, the underlying services still expect traffic that *looks like* a real browser session: a modern TLS handshake, a sensible HTTP/2 preface, and consistent header/cookie behaviour. Sending a raw `hyper` or stock `reqwest` request stands out due to unusual cipher suites, missing `Sec-CH-UA` headers, and lack of automatic user-agent rotation.

`msp-rust` uses [`wreq`](https://github.com/0x676e67/wreq) + [`wreq-util`](https://github.com/0x676e67/wreq-util) to emulate the full browser stack:

| Layer         | What's emulated                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------ |
| **TLS**       | Cipher suites, extensions (ALPN, ECH, GREASE, …), supported groups, signature algorithms, curves |
| **HTTP/2**    | Initial window size, header table size, max concurrent streams, frame ordering                   |
| **JA3 / JA4** | Built-in browser presets match real Chrome / Firefox handshakes                                  |
| **Headers**   | `User-Agent`, `Sec-CH-UA`, `Sec-Fetch-*`, `Accept-Language` are auto-injected per profile        |
| **Cookies**   | Native cookie store enabled by default — `Set-Cookie` from login is automatically replayed      |

---

## 🔌 Proxy Support & Network Isolation

To guarantee stealth, `msp-rust` implements strict **Network Identity Isolation**. It ensures an account never leaks its host IP and maintains location stickiness.

### Proxy Auto-Normalization

The builder automatically normalizes both standard proxy URIs and standard proxy provider strings (e.g. `host:port:user:pass`) into standard HTTP/SOCKS5 proxies:

```rust
let client = MspClient::builder()
    // Provider format: host:port:user:pass
    .proxy("12.214.164.15:10000:user123:pass123")
    .enforce_proxy(true) // Actively fails fast on proxy failure; prevents direct connection fallback
    .build()?;
```

---

## 💾 Persistent Session Storage

To avoid sending repeated `Password Grant` requests (which trigger brute-force or malicious login patterns), `msp-rust` allows you to export your entire session — including tokens, device parameters, and TLS profiles — and reload it instantly.

```rust
use msp_client::{MspClient, SessionState, BrowserBrand};
use std::fs;
use std::path::Path;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state_file = "session.json";

    let client = if Path::new(state_file).exists() {
        // 1. Restore the session directly from disk
        let json = fs::read_to_string(state_file)?;
        let state = SessionState::from_json(&json)?;

        let restored = MspClient::builder()
            .from_state(state)
            .build()?;

        // 2. Perform a silent token refresh if near expiry
        let session = restored.session().get().await?;
        if session.is_expired() {
            println!("[AUTH] Access token expired, performing silent refresh...");
            restored.auth().refresh().await?;
        }
        restored
    } else {
        // 1. Create a fresh proxy-pinned client
        let fresh = MspClient::builder()
            .random_profile(BrowserBrand::Chrome)
            .proxy("12.214.122.15:10000:user123:pass123")
            .enforce_proxy(true)
            .build()?;

        fresh.auth().login("username", "password", "FR").await?;

        // 2. Export state to file
        if let Some(state) = fresh.export_state().await {
            fs::write(state_file, state.to_json()?)?;
        }
        fresh
    };

    Ok(())
}
```

---

## 📡 WebSocket Event Hooks

You can subscribe to real-time server events via `client.events()`. Incoming Engine.IO WebSocket packets are automatically parsed into strongly-typed Rust structures.

```rust
use msp_client::{MspClient, MspEvent};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let client = MspClient::builder().build()?; // Setup with credentials or state...

    let mut rx = client.events();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            match event {
                MspEvent::PingResponse(p) => {
                    println!("[HEARTBEAT] id={}", p.ping_id);
                }
                MspEvent::MessageSent(m) => {
                    println!("[MESSAGE] {}: {}", m.sender_profile_id, m.message_body);
                }
                MspEvent::RelationshipRequestCreated(r) => {
                    println!("[FRIEND_REQUEST] from={}", r.requester_profile_id);
                }
                MspEvent::RelationshipRequestChanged(e) => {
                    println!("[RELATIONSHIP] {} -> {}", e.old_state, e.new_state);
                }
                MspEvent::PassiveRewardEarned(r) => {
                    println!("[REWARD] +{} XP (source={:?})", r.xp, r.source_profile_id);
                }
                MspEvent::Unknown { message_type, .. } => {
                    println!("[RAW_FRAME] type={message_type}");
                }
            }
        }
    });

    client.auth().login("username", "password", "FR").await?;
    tokio::signal::ctrl_c().await?;
    Ok(())
}
```

---

## 📚 API Reference

### `MspClientBuilder`

| Method                                  | Description                                                                 |
| --------------------------------------- | --------------------------------------------------------------------------- |
| `device_id(id)`                         | Set a fixed device ID; otherwise a random uppercase UUIDv4 is generated     |
| `profile(p)`                            | Pin a specific `wreq_util::Profile`                                         |
| `platform(p)`                           | Pin a specific `wreq_util::Platform`                                        |
| `random_platform()`                     | Pick OS uniformly at `build()` time                                         |
| `random_profile(brand)`                 | Pick browser profile uniformly from a safe list of `BrowserBrand`          |
| `proxy(url)`                            | Configures proxy. Supports `host:port:user:pass` strings                    |
| `enforce_proxy(bool)`                   | Force fail-fast proxy safety; prevents IP leaks on network drops            |
| `from_state(SessionState)`              | Restores client parameters and tokens dynamically from an exported state   |
| `build()` → `Result<MspClient>`         | Build the client                                                           |

### `MspClient`

| Accessor            | Returns                              | Description                                              |
| ------------------- | ------------------------------------ | -------------------------------------------------------- |
| `auth()`            | `AuthEndpoint`                       | Login, token management, WebSocket presence control      |
| `events()`          | `broadcast::Receiver<MspEvent>`      | Subscribe to real-time events                            |
| `export_state()`    | `Option<SessionState>`               | Save active sessions                                     |
| `messaging()`       | `MessagingEndpoint`                  | Direct messaging and historical parsing                  |
| `greetings()`       | `GreetingsEndpoint`                  | System autographs and premium greetings                  |
| `attributes()`      | `AttributesEndpoint`                 | Read and write game properties                           |
| `comments()`        | `CommentsEndpoint`                   | Post and manage UGC comments                             |
| `reservations()`    | `ReservationsEndpoint`              | Reserve chatroom / StarQuiz slots                        |
| `session()`         | `&SessionStore`                      | Raw access to the token/identity store                   |
| `profile()`         | `Profile`                            | Active browser emulation profile                         |
| `platform()`        | `Platform`                           | Active OS emulation platform                             |

### `AuthEndpoint`

- `login(username, password, region) -> Result<MspSession>`: complete auth grant and start presence loop.
- `refresh() -> Result<()>`: proactively rotates access tokens using stored refresh tokens.

### `MessagingEndpoint`

- `get_conversations(page, page_size) -> Result<ConversationPage>`: fetches list of active threads.
- `get_chat_history(conversation_id, page_size) -> Result<Vec<ChatMessage>>`: reads historical chat frames.
- `mark_conversation_as_read(conversation_id) -> Result<ConversationEntry>`: mark messages as read.
- `get_or_create_conversation(other_profile_id) -> Result<Conversation>`: find or open a 1:1 thread.
- `send_message(conversation_id, body) -> Result<MessageReceipt>`: send a chat message.

### `GreetingsEndpoint`

- `get_greeting_definitions() -> Result<Vec<GreetingDefinition>>`: all greeting types with costs, cooldowns, XP formulas.
- `send_greeting(greeting_type, profile_id) -> Result<SendGreetingResult>`: send a greeting (`Autograph`, `StarGreeting`, `LoveGreeting`, `RainbowGreeting`, `PartyGreeting`, `SuperStarGreeting`).

### `AttributesEndpoint`

- `get(profile_id) -> Result<ProfileAttributes>`: fetch attributes (pass `None` for self).
- `update_additional_data_key(key, value) -> Result<ProfileAttributes>`: read-modify-write a key in `additionalData`.

### `CommentsEndpoint`

- `post(thread_id, text) -> Result<SentComment>`: post a UGC comment.

### `ReservationsEndpoint`

- `chatroom(level, version) -> Result<RoomReservation>`: reserve a chatroom slot.
- `quiz() -> Result<RoomReservation>`: reserve a StarQuiz slot.

---

## 🧪 Error Handling

All endpoints return `Result<T, MspError>`. Variants:

| Variant          | When                                                                                |
| ---------------- | ----------------------------------------------------------------------------------- |
| `Authentication` | Login / token rejection                                                             |
| `NoSession`      | An endpoint was called before `login()` or after `clear()`                          |
| `Network`        | `wreq` transport error                                                              |
| `Json`           | `serde_json` decoding / encoding                                                    |
| `Jwt`            | JWT layout or `sub` claim missing                                                   |
| `Api`            | Server returned an error payload (`status`, `body`)                                |
| `Base64`         | JWT base64 decoding                                                                 |
| `UrlEncoded`     | `serde_urlencoded` failure                                                          |
| `InvalidProxy`   | Proxy URL couldn't be parsed or was bypassed with `enforce_proxy` active            |

---

## 🏎️ Performance Notes

The whole client is `Clone`-able: the HTTP engine is internally `Arc`'d, and `SessionStore` is `Arc<RwLock<…>>`. Spawn as many concurrent endpoint handles as you need.

`Cargo.toml` is optimized for automation:

- **Dev** — optimized dependencies for rapid testing cycles.
- **Release** — full Link-Time Optimization (`lto = "thin"`) enabled.
- **Production** — `lto = "fat"`, `codegen-units = 1`, `panic = "abort"` for shipped binaries.

---

## ⚠️ Disclaimer

This project is an **unofficial**, reverse-engineered client for the MovieStarPlanet 2 private API. It is provided for **educational and research purposes only**. Using it may violate the MSP2 Terms of Service and could result in account sanctions. The authors take no responsibility for any consequences arising from its use.
