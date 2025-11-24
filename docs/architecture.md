# Application Architecture

This document provides an overview of the current application architecture for D-Buddy and suggests potential improvements for future development.

## Current Setup

The application is structured into three main Rust modules:

-   `src/main.rs`: This is the core of the application. It contains:
    -   The main application entry point (`main` function).
    -   The main application state, held in the `App` struct.
    -   The main event loop, which handles user input and other events.
    -   The `Config` struct for styling and other constants.

-   `src/ui.rs`: This module is responsible for all UI rendering. It contains the `ui` function, which takes the application state and a `ratatui` frame and draws the UI. This provides a good separation of rendering logic from the application logic.

-   `src/bus.rs`: This module handles all interactions with D-Bus. It defines the `Item` struct, which is the a structured representation of a D-Bus message, and the `dbus_listener` function, which connects to the session or system bus and listens for messages.

## Suggested Improvements

The current architecture is simple and effective for the current feature set. However, as the application grows, the following improvements could be considered to enhance maintainability and scalability.

### Modularization

The `main.rs` file is becoming large and is responsible for too many concerns. It could be split into several modules:

-   `src/state.rs`: The `App` struct and its `Default` implementation could be moved here. This would consolidate all state management in one place.

-   `src/event.rs`: The event handling logic, which is currently a large `match` statement in the main loop, could be moved to this module. This could include functions for handling events in different modes (e.g., `handle_normal_mode_events`, `handle_filtering_mode_events`). This would also be a good place to define and manage key bindings.

-   `src/config.rs`: The `Config` struct could be moved to its own module. In the future, this module could be expanded to load configuration from a file, allowing users to customize colors and other settings.

### Alternative Architecture: Model-View-Update (MVU)

For a more significant refactoring, the application could be structured using the Model-View-Update (MVU) pattern, which is a popular and effective pattern for TUI applications.

-   **Model**: This would be the application's state, currently represented by the `App` struct.

-   **View**: This is the UI rendering logic, which is already well-separated in `src/ui.rs`.

-   **Update**: This would be a function or a set of functions that take the current `Model` and a `Msg` (message) and return a new `Model`. The `Msg` enum would represent all possible actions that can change the application state (e.g., `Quit`, `NextItem`, `ToggleDetails`, `UpdateFilter`).

The main loop would be simplified to:
1.  Wait for an event.
2.  Convert the event into a `Msg`.
3.  Call the `update` function with the current `Model` and the `Msg`.
4.  Call the `view` function with the new `Model`.

This pattern would make the application more modular, easier to test (the `update` function would be a pure function that can be tested in isolation), and easier to reason about as the complexity grows.
