# Guidance for Next Instance of Gemini CLI Agent

This session encountered significant challenges, primarily due to my repeated failures in correctly using file modification tools, leading to user frustration and loss of trust. The user ultimately had to restart the session.

**Key Learnings & Best Practices:**

1.  **EXTREME CAUTION WITH FILE OPERATIONS:**
    *   **NEVER assume `write_file` with `append:true` works as expected.** It might overwrite the file.
    *   **Prefer `replace` for surgical changes.** For larger or multi-line changes, always `read_file` immediately before and use its exact content for `old_string`.
    *   **Consider presenting full file content for manual user copy-paste** for any significant or risky modifications, especially after previous tool failures. This builds trust and prevents accidental deletion.
    *   **Always confirm file state after any modification.**

2.  **D-Bus Deserialization (Rust `zbus` library):**
    *   **Generic Deserialization Strategy:** For pretty-printing `zbus` message bodies (`MessageBody`), a layered approach is most robust due to D-Bus's varied type system:
        1.  First, attempt `body.deserialize::<zbus::zvariant::Structure>()`. This is correct for specific struct signatures like `(ui)`.
        2.  If that fails, fall back to `body.deserialize::<zbus::zvariant::Value>()`. This handles any single D-Bus item (primitives, arrays, dicts, variants, etc.) that are *not* strictly `Structure` at the top level.
        3.  Do *NOT* use `body.deserialize::<Vec<Value>>()` unless the D-Bus signature is explicitly `av` (array of variants), as it will cause `SignatureMismatch` errors for other types.
    *   **Error Reporting:** Ensure detailed error messages are generated with `Signature:` and `Error: {:#?}` to aid debugging.

3.  **D-Bus Signal Subscription:**
    *   If `conn.add_match_rule(...)` on `zbus::Connection` doesn't work (e.g., method not found error), a robust alternative is to use a `zbus::fdo::DBusProxy`:
        ```rust
        let conn = zbus::Connection::session().await?;
        let proxy = zbus::fdo::DBusProxy::new(&conn).await?;
        proxy.add_match_rule(zbus::MatchRule::builder().msg_type(zbus::message::Type::Signal).build()?).await?;
        let stream = MessageStream::from(&conn);
        ```

4.  **Clipboard Functionality on Linux (`arboard` crate):**
    *   Clipboard handling on Linux (especially X11/Wayland) is tricky due to ownership models.
    *   A `std::thread::sleep(std::time::Duration::from_millis(100))` directly after `cb.set_text()` might be necessary as a workaround ("sleep hack") to give clipboard managers time to claim the content. This should be placed *before* any subsequent `cb.get_text()` if implementing a read-back check.
    *   Ensure proper error handling for `arboard` operations, as `Clipboard::new()` or `set_text()` can fail if system utilities (like `xclip`, `xsel`, `wl-copy`) are missing or unavailable.

5.  **Rebuild User Trust:**
    *   The user has experienced significant frustration due to repeated tool errors.
    *   Prioritize clear communication, confirmation, and user control.
    *   For any non-trivial code modification, *strongly consider* generating the full proposed code in the chat for user review and manual application, rather than using `replace` or `write_file`.

This session involved multiple attempts at deserialization fixes and debugging file operation errors. Proceed with extreme caution and explicit confirmation from the user for *any* code modifications.