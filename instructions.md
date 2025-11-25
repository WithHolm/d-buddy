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
* all tasks are made like : "- {checkbox} [task description}". any indentation under is subtasks for the main task.
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
- [x] add serial to listview between timestamp and sender, reply_serial if is_reply  
- [x] Color serial in listview based on stream type (Session/System)
- [x] filtering should pop up as a text input field in the middle of screen.
   - [x] add keymaps for filtering during filter view: esc: clear filter, enter: apply filter, tab: autofilter- [x] BUGFIX before anything else! no keys will work untill i have pressed up or down in main view
- [x] add total count of messages at top of listview (example [session(10)|system(20)|both(30)])
- [] add a check of console height/width.. if its under 20 height or width, make it go clear with a message "Console is too small to display the application". this need to be in app config


### Grouping
- [x] currently it groups by sender. please allow to group by member, path, serial and NONE. use "g" to toggle grouping. it should come up as a selector box in the middle of the screen.
 - [x] group selector list should be able to be scrolled up and down using the arrow keys. (up/down arrow keys)
 - [x] selector is not active by default in group selection view. please make it always active.
- [x] the size of the box where you select grouping is currently relative to console/application height and width. this needs to be static based upon grouping size, so the box is just as high as there are items in the list.
- [x] a selecatble item in the grouping selection box looks like this currently ">   m: Member" can you change this to only be "> Member" ?
- [x] first time group selection box spawns it spawns without selector. if i activate it by pressing don/up, i will spawn the selector for future selections. please fix it so selector is always active

### Filtering 
- [x] filtering should pop up as a text input field in the middle of screen, one line high. 
   - [x] keymap should be in same place it is in main view (at the bottom)
- [x] if autofilter is activated, a new popup should take its place with the selection you can make and example of value it would use:
  - [x] sender: "sender"
  - [x] member: "member"
  - [x] path: "path"
  - [x] serial: "serial"
  - [x] reply_serial: "reply_serial"
- [x] remove autofilter from main view. only available in filter view
- [x] filter is not a satatic height box, make it one. (3 rows - top line with 'filter', middle with input box, bottom line)

### colouring and config:
- [x] all colouring should be accessible via a shared struct. this struct should have a color scheme for the app for easier implementation.

### Messages
- [x] is seemes like recipient and reply_serial is not a property that is captured properly. the message item in bus has this and it seemes like it should have some value, but all testing has shown that it is not captured properly.

### architecture
- [x] refactor the code to use a more modular architecture. this will make it easier to add new features and improve performance. info in docs/arcitecutre.md under modularisation

### Optionals
- [x] Option to toggle relative times (e.g., “2s ago”) which can help see event bursts more clearly.
 - [x] Option to toggle relative times (e.g., “0s” -> “59s” -> “1m” -> “59m” -> “1h” ->...) which can help see event bursts more clearly.
- [x] Support hierarchical grouping (e.g., first by sender, then by member). -> managed by having more grouping enabled at one time. not hierarchical, but flat
- [] Expand/collapse groups and show message counts to manage high-volume streams.
- [] allow full-text search inside message arguments or property values.
  - [] Highlight matches in the message details view for easier spotting.
- [] dump messages to a file + load messages from a file (just active.. if you have a filter, just dump the ones shown, or mabye have a selector at that point..)
- [x] "lighting strike" ticker besides messages. goes from bright to dark depending on the time (60 sec)
- [x] look up the actual application path + pid sending the message. in detaisl we can show full path + any arguments if present, but in list view we only need to show "{appname} (pid)"
- [x] revisit application setup, document in /docs about current setup and possible improvements (including new code files to be better organized or alterntiative architecture (keep it high level))


### Main view formatting

#### normal list
currently:    
```
[5m] 60496 sender: systemd (pid:1506), member: PropertiesChanged, path: /org/freedesktop/systemd1/unit/omarchy_2dbattery_2dmonitor_2eservice
```

suggested: 
``` 
[timestamp] [reply-glyph if is reply] [sender-app-name:pid-number|sender] [glyph_right_arrow] [receiver-app-name:pid-number|receiver] [member]@[path]
```

#### grouped list
as it stands now grouing takes one item (possibly the first in stack) as the "top level" item and then indents all other items under it.
suggested new grouping:
```
[group value]
   [item without the group value]
```

what is magical here is that when you are inside a the [group value] is sticky, meaning that it will be displayed even if you scroll up or down the list. 
if you are in between 2 values, the sticky value will still be shown for the top items:
example: basic sticky when we have scrolled past the first items in same group..
```
{not showed: more items in group 1}
[group value] [↑...↓]
   [item without the group value]
   [item without the group value]
   [item without the group value]
{not showed: more items in group 1}
```


exmple: sticky between groups:
```
{not showed: more items in group 1}
[group1 value] (count) [↑...]
  [item without the group value shown]
  [item without the group value shown]
  [item without the group value shown]
[group2 value] (2)
  [item without the group2 value shown]
  [item without the group2 value shown]
[group3 value] (count) [...↓]
  [item without the group3 value shown]
  [item without the group3 value shown]
{not shown: more items in group 3}
```

### Logging & Performance Analytics

To diagnose and address performance slowdowns with large datasets, a comprehensive logging and analytics layer should be implemented.

-   [x] **Introduce `tracing` framework**: Integrate the `tracing`, `tracing-subscriber`, and `tracing-appender` crates to provide asynchronous, non-blocking logging to a file.
    -   Logs will be written to `d-buddy.log` in the current directory.
    -   Logging can be enabled via a `--log` command-line flag.
    -   Log level can be configured using the `RUST_LOG` environment variable (e.g., `RUST_LOG=d_buddy=debug`).

-   [x] **Instrument Critical Code Paths**: Add `tracing` spans to measure the duration of key operations that are likely to be performance-sensitive.
    -   **Main Event Loop**: Measure the time taken for each iteration of the main `run` loop in `src/main.rs` to identify overall performance bottlenecks.
    -   **Message Processing**:
        -   Time taken to filter the message list based on user criteria.
        -   Time taken to sort the filtered messages.
        -   Time taken to generate the `ListItem` widgets for rendering (the `flat_map` operation).
    -   **UI Rendering**: Measure the duration of the `terminal.draw` call to isolate rendering-specific slowdowns.
    -   **D-Bus Listener**: In `src/bus.rs`, log the time taken for each `get_process_info` call to identify any slow D-Bus interactions, especially on cache misses.

-   **In-App Analytics View (Optional)**:
    -   Add a new, optional pane to the TUI (toggled by a key, e.g., `d` for diagnostics) that displays real-time performance metrics.
    -   Metrics to display could include:
        -   Time of the last main loop iteration (in ms).
        -   A moving average of the loop time.
        -   The total number of messages currently held in memory.
        -   The cache hit/miss ratio for process information lookups.



### performance fixes

-   **Optimize `rendering_list` performance**: This area (currently taking tens of µs to a few ms) involves significant string processing and `ratatui` `Span` creation, identified as a potential hotspot, especially with large datasets.
    -   **Reduce String Allocations/Clones**:
        -   Explore using `Cow<'_, str>` where feasible in `sender_display`, `receiver_display`, and other string-heavy parts to minimize `String` allocations.
        -   Review `format!` macros and other string manipulations to reduce intermediate `String` creations.
    -   [x] **Optimize `ratatui` Text Construction**:
        -   [x] Consider pre-allocating `Vec<Span>` with an estimated capacity before pushing multiple spans to reduce reallocations.
        -   [x] Implement conditional `Span` creation: avoid creating `Span`s for empty or non-visible elements.
    -   **(Advanced) Lazy Rendering**: Investigate rendering only the visible `ListItem`s within the scroll viewport rather than all filtered and sorted items, especially when dealing with extremely large message sets.
-   [x] Add more granular tracing to `run:main_loop`: The most critical next step is to identify what part of the run:main_loop is consuming the majority of the unaccounted ~240-250ms. This will involve adding additional tracing::info_span! macros to other significant code blocks within src/main.rs's main loop.
-   [x] Implement identified optimizations for `rendering_list`: Proceed with the optimizations already suggested in instructions.md, specifically focusing on Cow<'_, str> for string handling and pre-allocating Vec<Span> for ratatui Text Construction.
-   [x] Investigate `get_process_info` call patterns: Determine how frequently get_process_info is invoked within a single main_loop iteration. Even if individual calls are fast (microseconds), frequent calls could lead to significant cumulative overhead.
