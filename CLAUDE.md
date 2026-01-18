# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build & Run

```bash
cargo build                      # Build debug
cargo run                        # Run with default account
cargo run -- -a +1234567890      # Run with specific Signal account
```

No test suite exists yet.

## Architecture

Signal-TTY is a terminal messaging client for Signal, built with Rust. It communicates with Signal via `signal-cli` running in JSON-RPC mode as a subprocess.

### Layer Overview

**Main Loop** (`src/main.rs`): Async tokio event loop with 20ms poll cycle handling terminal events, incoming Signal messages (via broadcast channel), and background image loading.

**App State** (`src/app.rs`): Central state with conversations, selection index, focus mode (Conversations/Messages/Input), and input buffer. `ConversationView` wraps conversations with runtime state (loaded messages, scroll offset).

**Infrastructure** (`src/infrastructure/`):
- `signal/client.rs` - Wraps JSON-RPC, manages send/receive, spawns notification handler
- `jsonrpc/client.rs` - RPC with request/response matching via UUID
- `transport/stdio.rs` - Spawns `signal-cli jsonRpc` subprocess

**Storage** (`src/storage/`): SQLite at `~/.local/share/signal-tty/messages.db`. Lazy loading: messages loaded per-conversation when viewed, older messages on scroll (100 at a time).

**UI** (`src/ui/`): Ratatui/crossterm TUI with three panels - conversations list (left), messages (top-right), input (bottom-right).

**Images**: `image_cache.rs` loads attachments in background thread. `avatar.rs` manages contact avatars, clears cache on terminal focus regain (required for terminal image protocols).

### Data Flow

- **Sending**: Input → `pending_send` → `SignalClient.send_message()` → JSON-RPC → signal-cli → save to DB → update UI
- **Receiving**: signal-cli notification → broadcast channel → `handle_incoming_message()` → save to DB → redraw

### Key Patterns

- Dirty-flag rendering via `needs_redraw`
- `StatefulProtocol` (ratatui-image) requires cache clearing on focus regain for terminal images
- List scroll offset must use `ListState.offset()` after rendering to stay in sync with avatars

### Important Notes
- Don't write comments, let the code speak for itself
