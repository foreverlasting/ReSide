-- Persist each device's hardware model (e.g. "iPad13,4", "iPhone16,2") so the
-- device list can show a *correct, stable* model even when the live read is
-- unavailable or unreliable — notably over the Wi-Fi (netmuxd) path, where the
-- per-device lockdown read can return the wrong device's identity. Captured at
-- install time over USB (reliable) alongside name + ios_version.
ALTER TABLE devices ADD COLUMN product_type TEXT;
