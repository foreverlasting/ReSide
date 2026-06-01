-- Cache Developer Mode status so the Devices ladder can show it over Wi-Fi,
-- where a live read can't run (the device isn't on the system usbmuxd socket).
-- `developer_mode_checked_ts` is the unix time of the last time we *knew* the
-- status (a USB read, or a successful install/refresh); NULL means never known,
-- so the ladder can distinguish "off" from "not yet checked".
ALTER TABLE devices ADD COLUMN developer_mode_checked_ts INTEGER;

-- Backfill: a successful install requires Developer Mode on iOS 17.4+, so any
-- device that already has an installation had it on at install time. Seed the
-- cache from that so an already-set-up device shows Developer Mode immediately
-- over Wi-Fi instead of demanding a fresh USB read.
UPDATE devices
   SET developer_mode_enabled = 1,
       developer_mode_checked_ts = strftime('%s', 'now')
 WHERE udid IN (SELECT DISTINCT device_udid FROM installations);
