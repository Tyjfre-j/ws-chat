# ws-chat

[![CI](https://github.com/Tyjfre-j/ws-chat/actions/workflows/ci.yml/badge.svg)](https://github.com/Tyjfre-j/ws-chat/actions/workflows/ci.yml)

A real-time terminal chat application built with WebSockets and Rust.

The server is built with `axum` and `tokio`. The client is a terminal UI built with `ratatui`.

## Features

- Persistent WebSocket connections with a defined handshake: username, confirmation, room selection, then chat
- Real-time message broadcasting within rooms, including to the sender
- Multiple case-insensitive rooms with join and leave notifications visible to other participants
- Client UI status stages: Connecting, Entering Username, Confirming Username, Selecting Room, Joining Room, Chatting, and Disconnected
- Three built-in color themes (Catppuccin, Nord, Tokyo Night), cycled with `Ctrl+T`
- Automatic reconnection with exponential backoff: 1s, 2s, 4s, and so on, capped at 30s
- A bounded connection-attempt timeout, so a hung TCP handshake doesn't leave the client stuck on "Connecting" forever
- Retry interruption with `Esc`
- Client-side validation of usernames, room names, and chat messages (length and control characters) before sending, so obviously invalid input never leaves the client
- Graceful handling of malformed input, empty fields, case-insensitive duplicate usernames, unsupported binary frames, and unexpected disconnects; chat messages larger than 4 KiB receive an error without closing the connection
- Per-room broadcast channels with lag tolerance: a slow client skips missed messages instead of stalling the room
- Automatic room cleanup after the last participant leaves
- Structured logging (file + console) on both client and server via `tracing`

## Architecture

```text
+-------------+        WebSocket (JSON text frames)        +-------------+
|   Client    | <----------------------------------------> |   Server    |
|  (ratatui)  |                                            |   (axum)    |
+-------------+                                            +-------------+
```

The server runs one asynchronous task per connection in `handle_socket`. During chat, `tokio::select!` coordinates room broadcasts and incoming client messages. If either direction fails, the connection ends cleanly and the server releases the user's username and room subscription. One connection's failure never affects any other connection or the server process — Tokio isolates each connection's task.

The client uses one asynchronous event loop to race the WebSocket read stream against `crossterm::event::EventStream`, so incoming messages and keyboard input remain responsive at the same time.

### Shared (`protocol/`)

- `lib.rs` - the wire-message types (`ClientMessage`, `ServerMessage`, `ErrorCode`), the generic `Received<T>` enum used by both sides' receive loops, and shared validation limits/helpers (`MAX_USERNAME_LEN`, `MAX_ROOM_NAME_LEN`, `MAX_CHAT_MESSAGE_LEN`, `has_control_characters`)

Keeping `Received<T>` and the validation constants here (rather than duplicated per crate) means client and server always agree on limits and on how a received message's outcome is represented, without the `protocol` crate taking on any transport-specific dependency (`axum` or `tokio-tungstenite`).

### Server (`server/`)

- `handlers/` - connection lifecycle, handshake, room selection, chat handling, broadcasting, and cleanup, split by concern:
  - `connection.rs` - the `/ws` upgrade handler and overall per-connection flow
  - `username.rs` - username proposal, confirmation, and reservation
  - `room.rs` - room selection and validation
  - `chat.rs` - the chat loop: broadcasting and per-message validation
  - `mod.rs` - shared constants and the `notify_error` helper used across the other four files
- `net.rs` - axum WebSocket send/receive helpers (`send_server_message`, `receive_client_message`, `send_error`, `forward_broadcast_message`)
- `state.rs` - shared application state: active rooms and reserved usernames, behind `Arc<DashMap<...>>`
- `protocol.rs` - re-exports the shared wire types, `Received`, and the shared validation constants from `protocol/`
- `logging.rs` - file + console tracing setup

Each room uses a `tokio::sync::broadcast` channel with a capacity of 256. Joining a room subscribes a client to that channel, and chat messages are broadcast to all subscribers. If a client falls behind, `broadcast::error::RecvError::Lagged` is logged, the client is sent a `LaggedBehind` error naming how many messages it missed, and it continues from the newest available message. A closed channel or socket error ends the session.

Rooms are created lazily when a client joins a non-empty room name and removed after their last receiver is dropped. Room names are trimmed and normalized to lowercase, so `Lobby` and `lobby` refer to the same room. The room list is a snapshot of existing rooms, but clients may also enter a new non-empty room name to create it.

### Client (`client/`)

- `run.rs` - connection lifecycle, connect timeout, retry handling, and exponential backoff
- `events.rs` - keyboard handling, client-side input validation, and stage-dependent message creation
- `net.rs` - serialization and incoming server-message parsing (`send_client_message`, `receive_server_message`)
- `app.rs` - application state and server-message handling, including stage transitions
- `ui.rs` - status bar, ↑/↓ scrolling message log with cached line-wrapping, and a trailing-input viewport for long text
- `theme.rs` - the three built-in color palettes and the theme-cycling logic
- `protocol.rs` - re-exports the shared wire types, `Received`, and shared validation constants; adds client-only state (`ClientStage`, `RunOutcome`, `RetryOutcome`)
- `logging.rs` - file + console tracing setup

## Protocol

Messages are JSON text frames tagged with `type` and an optional `data` payload:

```json
{
  "type": "ChatMessage",
  "data": {
    "message": "hello everyone"
  }
}
```

### Client to server

| Type              | Payload                 | When                              |
| ----------------- | ------------------------ | --------------------------------- |
| `SetUsername`     | `{ "username": "..." }`  | Proposing a username              |
| `AcceptUsername`  | `{ "accepted": true }`   | Answering the confirmation prompt |
| `JoinRoom`        | `{ "room": "..." }`      | Choosing a room                   |
| `ChatMessage`     | `{ "message": "..." }`   | Sending a chat message            |

### Server to client

| Type                          | Payload                                      | When                                                     |
| ------------------------------ | ---------------------------------------------- | ---------------------------------------------------------- |
| `Welcome`                     | None                                          | Immediately after connecting                              |
| `ProposedUsername`            | `{ "username": "..." }`                       | Echoing the proposed username for confirmation            |
| `RoomList`                    | `{ "rooms": ["..."] }`                        | After the username is confirmed                           |
| `RoomJoinConfirmed`           | `{ "room": "..." }`                           | Confirming that the client joined its chosen room         |
| `ChatMessage`                 | `{ "username": "...", "message": "..." }`     | Broadcast to everyone in the room, including the sender   |
| `UserJoined` / `UserLeft`     | `{ "username": "..." }`                       | Presence notifications                                    |
| `Error`                       | `{ "code": "...", "message": "..." }`         | Validation or protocol errors                             |

- **Structured protocol errors:** Error responses include a stable machine-readable `code` and a human-readable `message`. The client uses the current UI stage — not just the error code — to decide how to recover, so a rejection received while joining a room always returns the client to room selection regardless of which specific code caused it. Username-related codes similarly return the client to entering a username.

## Running it

Requires Rust 1.85+ and Cargo (the workspace uses the 2024 edition).

From the repository root, start the server:

```sh
cargo run -p ws-chat-server
```

In another terminal, start one or more clients:

```sh
cargo run -p ws-chat-client
```

The server listens on `127.0.0.1:3000` by default.

- WebSocket endpoint: `ws://127.0.0.1:3000/ws`
- Health endpoint: `http://127.0.0.1:3000/health`

Configuration is optional and uses environment variables:

| Variable              | Default                    | Purpose                                      |
| ---------------------- | ---------------------------- | ----------------------------------------------- |
| `WS_CHAT_SERVER_ADDR`  | `127.0.0.1:3000`             | Address on which the server listens             |
| `WS_CHAT_SERVER_URL`   | `ws://127.0.0.1:3000/ws`     | WebSocket URL used by the client                |
| `WS_CHAT_LOG_DIR`      | `logs`                       | Directory for client and server log files       |

Run multiple client instances in separate terminals to chat between them. Since rooms are created lazily, the first client can enter a new non-empty room name; later clients will see that room in their room list.

**Logging note:** log files roll daily and are only guaranteed to be flushed to disk when the process exits cleanly (`Esc` to quit, or `Ctrl+C` on the server, both of which trigger graceful shutdown). Force-killing either process can lose whatever was still buffered.

## Development checks

The CI pipeline runs the following; running them locally before pushing catches most failures early:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace --verbose
```

## Design decisions

- **Server-authoritative chat:** The client does not locally echo sent messages. It waits for the server to broadcast the message back, so every participant receives the same server-generated message.
- **Two-tier message-size limits:** The server rejects chat text larger than 4 KiB with an error while keeping the connection open. Axum also applies a 64 KiB maximum WebSocket message size as a protocol-level backstop; exceeding that limit ends the connection.
- **Client-side validation mirrors server-side validation:** Length and control-character checks run identically on both sides, using the same shared constants from `protocol/`, so the client can reject obviously invalid input instantly rather than waiting on a round trip — while the server still enforces every rule independently, since it never trusts the client.
- **Reconnection never gives up:** Failed connection attempts use exponential backoff capped at 30 seconds. After a connection was established and later disconnected, the next retry starts at 1 second. There is no maximum retry count; the client keeps trying until the user quits with `Esc`.
- **Control frames are separate from application messages:** Ping and pong frames are ignored by the application rather than parsed as JSON. Binary frames are rejected, and close frames end the session. The server and client do not implement a separate application-level heartbeat.
- **Task-per-connection isolation:** Each server connection runs as an independent async task. A panic, protocol error, or disconnect in one connection cannot affect another connection or the server process as a whole.

## Known limitations

- No message persistence. History exists only in memory for the current process and is not visible to clients who join later.
- No authentication. Usernames are reserved globally while their connections are active, but there is no identity verification.
- Single server instance. There is no cross-server message delivery. Horizontal scaling would require shared messaging infrastructure such as Redis Pub/Sub.
- No typing indicators.
- Room lists are snapshots sent during the handshake; clients are not notified when rooms are created or removed afterward.
- The username/room-selection handshake itself has no timeout once a connection is established, so an idle client can hold a connection open indefinitely during setup. (The initial TCP/WebSocket connection attempt does have a bounded timeout — see Features above.)
- Client input is not restored after a locally validated message is sent but then rejected by the server (e.g. a room name that passes client-side checks but is rejected by the server). The field is left empty and the user re-enters the value. This was a deliberate simplicity trade-off, not an oversight.
- A malformed or unparseable message received from the server ends the client's connection (triggering the normal reconnect-with-backoff flow) rather than attempting to skip just that one message and continue. This keeps the client from running with protocol state it can't fully trust, at the cost of a full reconnect for what might be a single corrupted frame.
- No automated tests are currently included.

## License

MIT License — see [LICENSE](LICENSE) for the full text.
