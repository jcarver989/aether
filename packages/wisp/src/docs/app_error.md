Application-level errors for the Wisp TUI.

# Variants

- **`Io`** — wraps [`std::io::Error`] from terminal I/O (raw mode, rendering).
- **`Acp`** — wraps [`AcpClientError`](acp_utils::client::AcpClientError) from the agent connection (spawn failure, protocol errors, broken pipe).

Both variants implement `From` conversions so `?` propagation works naturally.
