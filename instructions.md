# D-Buddy

A TUI for browsing D-Bus message streams using Rust, `ratatui`, and `zbus`.

## Current State & Architecture

The application has been recently refactored to support multiple D-Bus streams (Session and System) concurrently.

- **`main.rs`**: Contains the main application struct (`App`), state management, the primary event loop, and all UI rendering logic.
- **`bus.rs`**: Defines the data handling for D-Bus connections.
  - It introduces a clean `Item` struct that provides a structured representation of a message, including metadata like sender, destination, and reply info.
  - It provides an async `dbus_listener` function that connects to a specified bus (Session or System), spawns a background task to listen for signals, and populates a shared vector of `Item`s.
- **Data Model**: The central `App` struct now holds a `HashMap` where keys are the `BusType` (Session/System) and values are `Arc<tokio::sync::Mutex<Vec<bus::Item>>>`. This allows each bus listener to independently and safely update its own list of messages.

**Key Discrepancies:**
1.  **Disconnected Data**: The new `dbus_listener` function is not yet called in `main.rs`. The application currently starts with no active D-Bus connection and receives no live data.
2.  **Outdated UI Code**: The rendering logic in `main.rs` has not been updated to work with the new `HashMap` data structure and will cause errors.

---

## Roadmap

This plan outlines the next steps to integrate the new architecture and build out features.

### 1. Core Integration & Refactoring
- [ ] **Connect Data Sources**: In `main.rs`, call the `bus::dbus_listener` for both the Session and System buses on startup, populating the `app.messages` HashMap.
- [ ] **Fix UI Rendering**: Update the UI code to correctly read messages from the `Vec<Item>` corresponding to the currently active bus type (`app.stream`).
- [ ] **Decouple UI**: Move all UI-related functions and logic (e.g., `ui`, `centered_rect`) from `main.rs` into `src/ui.rs` to improve separation of concerns.

### 2. Multi-Bus View & Navigation
- [ ] **Implement Bus Switching**: Add a keybinding (e.g., `Tab`) to cycle the `app.stream` state between `Session` and `System` views.
- [ ] **Update UI Title**: The UI should clearly indicate which bus (`Session` or `System`) is currently being displayed.
- [ ] **(Optional) Combined View**: Consider a `Both` view that merges messages from all streams, sorted chronologically.

### 3. UI/UX Enhancements
- [ ] **Add Colors**: Use colors to differentiate elements like the sender, member, and path in the message list for better readability.
- [ ] **Group Messages**: Use the `serial` and `reply_serial` fields in `bus::Item` to visually group conversations. For example, indenting a reply message under its original request.

### 4. Advanced Interactivity
- [ ] **Auto-Filter**: Create a keybinding that, when pressed on a message, automatically populates the filter with that message's sender, creating an instant "conversation view".
- [ ] **Reply Functionality**: Explore implementing a "reply" feature. A first step could be generating a `dbus-send` command template based on the selected message and copying it to the clipboard.

### 5. Code Health & Suggestions
- [ ] **Consolidate `BusType`**: Merge the duplicate `BusType` enums from `main.rs` and `bus.rs` into a single definition in `bus.rs`.
- [ ] **Add `Clear` command**: Implement a keybinding to clear the message list for the current view.
- [ ] **Advanced Filtering**: Extend the filtering capabilities beyond a simple text search to allow filtering by specific fields (e.g., `member=NameAcquired`, `path=/org/freedesktop/DBus`).
