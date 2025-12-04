# D-Buddy Task and Development Guide

This document provides an overview of the `d-buddy` TUI application, guidelines for development, and an analysis of the existing codebase for potential improvements.

## 1. Overview

`d-buddy` is a terminal-based user interface (TUI) for monitoring D-Bus messages on Linux systems. It captures session and system bus messages in real-time and presents them in a scrollable, filterable, and groupable list. It allows developers to inspect the details of D-Bus signals, method calls, and replies, making it a useful tool for debugging D-Bus-related applications.

The application is built in Rust using `ratatui` for the TUI, `tokio` for asynchronous operations, and `zbus` for D-Bus communication.

## 2. Usage

### Running the Application

You can run the application directly using Cargo:

```bash
# Basic run
cargo run

# Enable logging to a file (d-buddy.log)
cargo run -- --log

# Enable the debug UI to see item counts and selection state
cargo run -- --debug-ui
```

### Keybindings

The application has several modes, each with its own set of keybindings.

#### Normal Mode (Main View)

-   `q`: Quit the application.
-   `↑`/`↓`: Navigate the message list.
-   `s` / `space`: Open the details view for the selected message.
-   `f`: Enter **Filtering Mode**.
-   `g`: Enter **Grouping Selection Mode**.
-   `Tab`: Cycle between message streams (`Session`, `System`, `Both`).
-   `t`: Toggle between relative and absolute timestamps.
-   `x`: Enter **Thread View** for the selected message, showing the original call and its reply.
-   `r`: Copy a `dbus-send` command template for the selected message to the clipboard.

#### Details View

-   `q` / `s` / `Esc`: Close the details view and return to the normal list.
-   `c`: Copy the full content of the details view to the clipboard and save it to `/tmp/d-buddy-details.txt`.
-   `j`/`k`: Scroll down/up one line.
-   `PgDn`/`PgUp`: Scroll down/up 10 lines.

#### Filtering Mode

-   `Esc`: Exit filtering mode and clear the filter.
-   `Enter`: Apply the filter and return to Normal Mode.
    -   `some text`: Filters for messages containing "some text" in the sender, member, or path.
    -   `sender=app_name`: Filters for messages where the sender name contains "app_name".
    -   `member=SignalName`: Filters for messages where the member name contains "SignalName".
    -   To clear a specific filter, type `sender=` and press Enter.
-   `Tab`: Enter **Auto-Filter Selection Mode** to populate the filter from the currently selected message.

---

## 3. Development Guidelines

### Setup

1.  **Build:** `cargo build`
2.  **Check:** `cargo check` (run this frequently)
3.  **Lint:** `cargo clippy` (run this before committing)

### Workflow
<!--AI NO TOUCH ZONE-->
1.  **Before Making a Change:**
    -   Run `make ai-check` to ensure there are no existing issues.

2.  **After Making a Change:**
    -   Run `make ai-check` again to catch any new warnings or errors.
    -   Run the application (`make ai-run`) to manually test your changes.
    -   Consider adding automated tests if you are adding new, testable logic.

### Code Structure

-   `main.rs`: The application entry point, main event loop, and terminal setup.
-   `bus.rs`: Handles all D-Bus connection logic, message listening, and data structuring (`Item`).
-   `ui.rs`: Contains all rendering logic for the `ratatui` interface.
-   `event.rs`: Handles all user input (key presses) and application state changes.
-   `state.rs`: Defines the core application state (`App` struct) and modes (`Mode` enum).
-   `config.rs`: Defines the color scheme and other configuration constants.

---

## 4. Task List & Code Improvements

This section tracks ongoing and completed refactoring and bug-fix tasks.

### Done

-   **[High] Performance: Fix Message List Cloning in Main Loop**
    -   **Issue:** On every UI tick, the entire message list was cloned from a shared `Mutex`, causing high CPU usage and UI lag.
    -   **Fix:** Refactored the data flow to use MPSC channels. The D-Bus listener threads now act as producers, pushing messages to a channel. The main UI loop consumes these messages without expensive locking or cloning the entire history on every frame.

-   **[High] Bug: Fix Unbounded Memory Growth**
    -   **Issue:** The source vectors holding all session and system messages were growing indefinitely because the `max_messages` constraint was not being applied to them.
    -   **Fix:** After new messages are received from the channels, the source vectors (`all_session_items` and `all_system_items`) are now trimmed to respect the `config.max_messages` limit.

-   **[Medium] Refactoring: Encapsulate App State**
    -   **Issue:** The `App` struct exposed its internal MPSC receivers and message storage vectors (`Vec<Item>`) as public fields, breaking encapsulation.
    -   **Fix:** Made the receiver and vector fields private. Added public accessor methods (e.g., `poll_session_messages`, `get_session_items`, `add_session_item`) to provide controlled access to the state.

### To Do

-   **[Medium] Refactoring: Reduce Code Complexity**
    -   **Issue:** The main `run` loop in `main.rs` and the `handle_event` function in `event.rs` are very large and contain complex, nested logic.
    -   **Recommendation:** Refactor these into smaller, more focused functions (e.g., `handle_normal_mode_event`, `handle_filtering_mode_event`).

-   **[Medium] Robustness: Remove `unwrap()` Calls**
    -   **Issue:** The code contains several `.unwrap()` calls which can lead to panics.
    -   **Recommendation:** Replace `.unwrap()` with `if let Some(...)` or `match` statements for graceful error handling.

-   **[Low] Performance: Optimize "Both" View Clone**
    -   **Issue:** The view for `BusType::Both` still clones all items from the session and system vectors to create a combined list for filtering and display.
    -   **Recommendation:** Refactor the processing logic to build a `Vec<&Item>` from the source vectors to avoid cloning all `Item` objects. The final, much smaller, filtered list can then be cloned before being stored in `app.filtered_and_sorted_items`.

-   **[Low] Robustness: Prevent Potential Stack Overflow**
    -   **Issue:** The recursive `format_value` function in `ui.rs` could cause a stack overflow on very deeply nested D-Bus messages.
    -   **Recommendation:** Introduce a depth limit to the recursion.
