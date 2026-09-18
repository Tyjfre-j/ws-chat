# ws-chat

A real-time terminal chat application built with WebSockets and Rust.

The server is built with `axum` and `tokio`. The client is a terminal UI built with `ratatui`.

## Features

- Persistent WebSocket connections with a defined handshake: username, confirmation, room selection, then chat
- Real-time message broadcasting within rooms, including to the sender
- Multiple rooms with join and leave notifications visible to other participants
- Client UI status stages: Connecting, Entering Username, Confirming Username, Selecting Room, Chatting, and Disconnected
- Automatic reconnection with exponential backoff: 1s, 2s, 4s, and so on, capped at 30s
- Retry interruption with `Esc`
- Graceful handling of malformed input, empty fields, duplicate usernames, and unexpected disconnects; oversized chat requests up to 4 KiB receive an error without closing the connection
- Per-room broadcast channels with lag tolerance: a slow client skips missed messages instead of stalling the room
- Automatic room cleanup after the last participant leaves

## Architecture

```text
+-------------+        WebSocket (JSON text frames)        +-------------+
|   Client    | <----------------------------------------> |   Server    |
|  (ratatui)  |                                            |   (axum)    |
+-------------+                                            +-------------+
```

The server runs one asynchronous task per connection in `handle_socket`. During chat, `tokio::select!` coordinates room broadcasts and incoming client messages. If either direction fails, the connection ends cleanly and the server releases the user's username and room subscription.

The client uses one asynchronous event loop to race the WebSocket read stream against `crossterm::event::EventStream`, so incoming messages and keyboard input remain responsive at the same time.

### Server (`server/`)

- `handlers.rs` - connection lifecycle, handshake, room selection, chat handling, broadcasting, and cleanup
- `state.rs` - shared application state: active rooms and reserved usernames
- `protocol.rs` - server and client wire-message definitions

Each room uses a `tokio::sync::broadcast` channel with a capacity of 256. Joining a room subscribes a client to that channel, and chat messages are broadcast to all subscribers. If a client falls behind, `broadcast::error::RecvError::Lagged` is logged and the client continues from the newest available message. A closed channel or socket error ends the session.

Rooms are created lazily when a client joins a non-empty room name and removed after their last receiver is dropped. The room list is a snapshot of existing rooms, but clients may also enter a new non-empty room name to create it.

### Client (`client/`)

- `run.rs` - connection lifecycle, retry handling, and exponential backoff
- `events.rs` - keyboard handling and stage-dependent message creation
- `net.rs` - serialization and incoming server-message dispatch
- `app.rs` - application state and server-message handling
- `ui.rs` - status bar, scrolling message log, and input box rendering

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
| ----------------- | ----------------------- | --------------------------------- |
| `SetUsername`     | `{ "username": "..." }` | Proposing a username              |
| `ConfirmUsername` | `{ "confirmed": true }` | Answering the confirmation prompt |
| `JoinRoom`        | `{ "room": "..." }`     | Choosing a room                   |
| `ChatMessage`     | `{ "message": "..." }`  | Sending a chat message            |

### Server to client

| Type                      | Payload                                   | When                                                    |
| ------------------------- | ----------------------------------------- | ------------------------------------------------------- |
| `Welcome`                 | None                                      | Immediately after connecting                            |
| `ConfirmUsername`         | `{ "username": "..." }`                   | Echoing the proposed username for confirmation          |
| `RoomList`                | `{ "rooms": ["..."] }`                    | After the username is confirmed                         |
| `ChatMessage`             | `{ "username": "...", "message": "..." }` | Broadcast to everyone in the room, including the sender |
| `JoinedRoom` / `LeftRoom` | `{ "username": "..." }`                   | Presence notifications                                  |
| `Error`                   | `{ "code": "...", "message": "..." }`     | Validation or protocol errors                           |

- **Structured protocol errors:** Error responses include a stable machine-readable `code` and a human-readable `message`. The client uses the code for state transitions and the message for display.

## Running it

Requires stable Rust and Cargo.

From the repository root, start the server:

```sh
cargo run -p ws-chat-server
```

In another terminal, start one or more clients:

```sh
cargo run -p client
```

The server listens on `127.0.0.1:3000`.

- WebSocket endpoint: `ws://127.0.0.1:3000/ws`
- Health endpoint: `http://127.0.0.1:3000/health`

Run multiple client instances in separate terminals to chat between them. Since rooms are created lazily, the first client can enter a new non-empty room name; later clients will see that room in their room list.

## Design decisions

- **Server-authoritative chat:** The client does not locally echo sent messages. It waits for the server to broadcast the message back, so every participant receives the same server-generated message.
- **Two-tier message-size limits:** The server rejects incoming chat JSON text larger than 4 KiB with an error while keeping the connection open. Axum also applies a 64 KiB maximum WebSocket message size as a protocol-level backstop; exceeding that limit ends the connection. The 4 KiB application check applies to the serialized request, not only the value of the `message` field.
- **Reconnection never gives up:** Failed connection attempts use exponential backoff capped at 30 seconds. After a connection was established and later disconnected, the next retry starts at 1 second.
- **Control frames are separate from application messages:** Ping and pong frames are ignored by the application rather than parsed as JSON. Binary frames are rejected, and close frames end the session. The server and client do not implement a separate application-level heartbeat.

## Known limitations

- No message persistence. History exists only in memory for the current process and is not visible to clients who join later.
- No authentication. Usernames are reserved globally while their connections are active, but there is no identity verification.
- Single server instance. There is no cross-server message delivery. Horizontal scaling would require shared messaging infrastructure such as Redis Pub/Sub.
- No typing indicators.
- Room lists are snapshots sent during the handshake; clients are not notified when rooms are created or removed afterward.
- No automated tests are currently included.
