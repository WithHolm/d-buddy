# D-Buddy

A TUI for browsing D-Bus message streams using Rust, `ratatui`, and `zbus`.

## Current State & Architecture

The application has been recently refactored to support multiple D-Bus streams (Session and System) concurrently.

- **`main.rs`**: Contains the main application struct (`App`), state management, the primary event loop, and all UI rendering logic.
- **`bus.rs`**: Defines the data handling for D-Bus connections.
  - It introduces a clean `Item` struct that provides a structured representation of a message, including metadata like sender, destination, and reply info.
  - It provides an async `dbus_listener` function that connects to a specified bus (Session or System), spawns a background task to listen for signals, and populates a shared vector of `Item`s.
- **Data Model**: The central `App` struct now holds a `HashMap` where keys are the `BusType` (Session/System) and values are `Arc<tokio::sync::Mutex<Vec<bus::Item>>>`. This allows each bus listener to independently and safely update its own list of messages.

## AI
* AI before it can change code, create a new file at root called current, with what it needs to do and recomended steps (your plan). you can afterwards replace all content inside this file every time you want to update the codebase.
* please stop and wait after every step. dont just run through everything autonomously.
* update this file when a task has been completed.
* dont run git command, remind me to commit changes after each task completion.
* NEVER RUN ANY GIT COMMANDS. dont commit any changes
* dont run cargo clippy 
* always run cargo check and cargo fmt after code changes

---

## Roadmap

This plan outlines the next steps to integrate the new architecture and build out features.

### 1. Core Integration & Refactoring
- [x] **Connect Data Sources**: In `main.rs`, call the `bus::dbus_listener` for both the Session and System buses on startup, populating the `app.messages` HashMap.
- [x] **Fix UI Rendering**: Update the UI code to correctly read messages from the `Vec<Item>` corresponding to the currently active bus type (`app.stream`).
- [x] **Decouple UI**: Move all UI-related functions and logic (e.g., `ui`, `centered_rect`) from `main.rs` into `src/ui.rs` to improve separation of concerns.

### 2. Multi-Bus View & Navigation
- [x] **Implement Bus Switching**: Add a keybinding (e.g., `Tab`) to cycle the `app.stream` state between `Session` and `System` views.
- [x] **Update UI Title**: The UI should clearly indicate which bus (`Session` or `System`) is currently being displayed.
- [x] (Optional) Combined View: Consider a `Both` view that merges messages from all streams, sorted chronologically.

### 3. UI/UX Enhancements
- [x] **Add Colors**: Use colors to differentiate elements like the sender, member, and path in the message list for better readability.
- [x] **Group Messages**: Messages are now grouped by sender and indented for clarity.
- [x] **Improved array display in details view**: Arrays of `u8` now show a string representation, and simple arrays are displayed compactly on one line.

### 4. Advanced Interactivity
- [x] **Auto-Filter**: Create a keybinding that, when pressed on a message, automatically populates the filter with that message's sender, creating an instant "conversation view".
- [x] **Reply Functionality**: Explore implementing a "reply" feature. A first step could be generating a `dbus-send` command template based on the selected message and copying it to the clipboard.

### 5. Code Health & Suggestions
- [x] **Consolidate `BusType`**: Merge the duplicate `BusType` enums from `main.rs` and `bus.rs` into a single definition in `bus.rs`.
- [x] **Add `Clear` command**: Implement a keybinding to clear the message list for the current view.
- [x] **Advanced Filtering**: Extend the filtering capabilities beyond a simple text search to allow filtering by specific fields (e.g., `member=NameAcquired`, `path=/org/freedesktop/DBus`).
- [x] **Add `--check` mode**: Add a command line flag to run the app without the TUI for testing.


### 6. UI/UX Enhancements
- [x] Main window can switch between `Session` and `System` buses, but it only shows the active one: please implement a visual indication that there is more. thinking [*session*|**system**] to indicate system is active.
- [x] the > field is only shown after you have pressed up or down to select messages
- [x] change filtering key from / to f
- [x] enhance the autofiler selection, where you can select sender, member, path, serial
- [x] ability to drill down and get the whole "thread" if 2 applications talk to eachother. each item has serial and reply serial. need to filter so every message and reply is shown.. this "drill down" need to be available from both main ui and details view.
- [x] remove color from main view if details pane is open
- [x] add is_reply to header in details view. this also needs to be shown in main view with a simple glyph 
   - example: [timetamp] [glyph if is_reply] ....rest of data like it is now
- [x] fix header in details view: it should be at top besides "Message Details [...↓]" suggestion "Message details [data above or below thingy] {sender} -> {recipient}|{serial}->{reply_serial if is_reply}|{member}:{path}"
   - [x] details pane: this can be on the line below "details view".
   - [x] details pane: remove the --- Header --- and header info below it
- [x] currently you can scroll past the bottom of details view. make sure to disable scroll down or up if the bottom/top is reached.
- [] add serial to listview between timestamp and sender, reply_serial if is_reply  
- [] filtering should pop up as a text input field in the middle of screen. 
   - [] add keymaps for filtering during filter view: esc: clear filter, enter: apply filter, tab: autofilter
- [x] BUGFIX before anything else! no keys will work untill i have pressed up or down in main view
### Grouping
- [x] currently it groups by sender. please allow to group by member, path, serial and NONE. use "g" to toggle grouping. it should come up as a selector box in the middle of the screen.
 - [] group selector list should be able to be scrolled up and down using the arrow keys. (up/down arrow keys)
