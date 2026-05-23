//! Background agent: generates + manages the `reside-agent.{service,timer}` and
//! `reside-tunneld.service` systemd user units, with an XDG autostart fallback
//! for non-systemd hosts. Yields to the UI via the process file lock. Phase 4.
