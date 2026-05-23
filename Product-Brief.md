## tl;dr

Build a Linux-first desktop app that lets users sign, sideload, and automatically refresh iOS apps on their own iPhone or iPad using USB or Wi-Fi. The key differentiator is reliable background refresh over Wi-Fi before the 7-day free Apple ID signing window expires, with Arch Linux as the primary target and broader Linux support as a stretch goal.

## Goals

### Business Goals

* Ship a working MVP for Arch-based Linux users that can sign and sideload an IPA over USB with at least 90% successful completion in internal testing.
* Achieve reliable Wi-Fi refresh automation with at least 85% refresh success rate when the paired device is online on the same network.
* Reduce manual weekly resigning effort from 10-20 minutes to under 1 minute of active user effort after initial setup.
* Build a Linux-native alternative to Windows-first tools like Sideloadly, measured by at least 1,000 GitHub stars or 500 active installs within 6 months of public release.
* Maintain user trust by keeping Apple ID credentials local-only and achieving zero known credential leakage incidents.

### User Goals

* Sideload an IPA onto a personally owned iPhone or iPad from Linux without needing a Windows VM or separate Mac.
* Connect to the device by USB for initial setup, pairing, installation, and troubleshooting.
* Enable Wi-Fi-based device communication after pairing so app refreshes can happen automatically.
* Sign apps using a free Apple ID or paid developer account, with clear guidance on limits and expiration windows.
* Avoid surprise app expiration by receiving notifications and automatic refresh attempts before the 7-day signing period ends.

### Non-Goals

* Do not support piracy, DRM bypass, cracked apps, stolen developer certificates, or sideloading apps the user does not have the right to install.
* Do not attempt to jailbreak devices or bypass iOS security protections.
* Do not guarantee compatibility with every iOS version, every IPA, or every Linux distribution in the first release.

## User Stories

Arch Linux power user

* As an Arch Linux user, I want to install the app from AUR or a packaged release, so that I can start sideloading without compiling multiple dependencies manually.
* As an Arch Linux user, I want the app to detect required system services like `usbmuxd`, so that I can fix setup issues quickly.
* As an Arch Linux user, I want logs and advanced diagnostics, so that I can troubleshoot pairing, signing, and installation failures.

Free Apple ID user

* As a free Apple ID user, I want to sign an IPA with my Apple ID, so that I can install it on my personal device without a paid developer account.
* As a free Apple ID user, I want the app to clearly show when each installed app will expire, so that I know when refresh is needed.
* As a free Apple ID user, I want automatic Wi-Fi refresh before expiration, so that my apps continue working without weekly manual work.
* As a free Apple ID user, I want the app to explain free account limitations, so that I understand why refreshes are needed every 6-7 days.

Paid Apple Developer account user

* As a paid developer account user, I want to use my certificate and provisioning profile, so that I can sign apps with longer validity.
* As a paid developer account user, I want to manage multiple devices and bundle IDs, so that I can sideload across my own test devices.

Privacy-conscious user

* As a privacy-conscious user, I want Apple ID credentials and signing assets stored locally, so that sensitive account information is not sent to a third-party server.
* As a privacy-conscious user, I want the option to avoid saving credentials, so that I can re-authenticate manually when needed.

## Functional Requirements

* Device Connection (Priority: P0)

  * USB Device Detection: Detect connected iPhone and iPad devices using `libimobiledevice` and `usbmuxd`, showing device name, UDID, iOS version, trust status, and connection type.
  * Device Pairing: Guide the user through iOS trust prompts and persist pairing records locally so future sessions can connect without repeating setup.
  * Wi-Fi Device Discovery: Detect previously paired devices available over Wi-Fi on the same local network when iOS Wi-Fi sync/network pairing is available.
  * Connection Health Status: Show whether the device is reachable, locked, trusted, busy, offline, or unsupported.

* IPA Import and Validation (Priority: P0)

  * IPA File Picker: Let users select an `.ipa` file from local storage using drag-and-drop or file picker.
  * IPA Metadata Extraction: Read app name, bundle identifier, version, icon, minimum iOS version, and entitlements from the IPA.
  * Compatibility Check: Warn users when the app may be incompatible due to iOS version, device architecture, missing entitlements, or invalid IPA structure.

* Signing (Priority: P0)

  * Free Apple ID Signing Flow: Support signing using a user-provided Apple ID where technically feasible, including 2FA/app-specific password handling if required by Apple authentication flows.
  * Paid Developer Signing Flow: Support importing existing certificates and provisioning profiles for users with paid developer accounts.
  * Local Signing Assets: Store certificates, provisioning profiles, and related metadata locally using OS keyring support where possible.
  * Bundle ID Management: Allow the user to reuse, modify, or generate app-specific bundle identifiers when required by signing constraints.
  * Entitlements Handling: Preserve safe entitlements where possible and remove or warn on unsupported entitlements for free Apple ID signing.
  * Signing Validation: Confirm the signed IPA has a valid code signature before installation begins.

* Sideloading and Installation (Priority: P0)

  * USB Install: Install a signed IPA to a connected iOS device over USB.
  * Installation Progress: Show progress stages including preparing IPA, signing, transferring, installing, and verifying installation.
  * Installation Result: Show clear success or failure messages with actionable remediation.
  * Installed App Inventory: Display apps installed through this tool, including app name, bundle ID, install date, signing identity, expiration date, and refresh status.

* Automatic Refresh (Priority: P0)

  * Expiration Tracking: Track signing expiration per installed app and calculate the recommended refresh window.
  * Refresh Scheduler: Schedule automatic refresh attempts 24-48 hours before expiration, with configurable timing.
  * Wi-Fi Refresh: When the device is reachable over Wi-Fi, re-sign and reinstall the app without requiring a USB connection.
  * Retry Logic: Retry failed refreshes with backoff and notify the user if manual action is needed.
  * Background Agent: Run a lightweight local background service or desktop autostart process to perform refresh checks even when the main UI is closed.

* Notifications and Status (Priority: P1)

  * Desktop Notifications: Notify users about upcoming expiration, successful refresh, failed refresh, device offline, and credential/action required states.
  * Dashboard: Show device status, app expiration timeline, recent refresh attempts, and next scheduled refresh.
  * Activity Log: Provide a user-readable timeline of pairing, signing, installation, and refresh events.

* Linux Packaging and Setup (Priority: P1)

  * Arch Package: Provide an Arch-first install path through AUR or a pacman-compatible package.
  * Cross-Distro Packages: Provide AppImage or Flatpak as a broader Linux distribution option if device access permissions can be handled reliably.
  * Dependency Checker: Detect missing dependencies such as `usbmuxd`, `libimobiledevice`, `ideviceinstaller`, `openssl`, and signing tools.
  * Permissions Setup: Guide the user through udev rules, group membership, and service startup requirements.

* Advanced Features (Priority: P2)

  * CLI Mode: Provide commands for pairing, signing, installing, listing devices, and refreshing apps.
  * Multi-Device Support: Manage multiple paired devices and assign different apps to different devices.
  * Backup and Restore: Export/import app configuration excluding sensitive credentials unless explicitly encrypted.
  * Experimental Direct Library Integration: Replace shelling out to command-line tools with direct Rust bindings where stable and maintainable.

## User Experience

Entry Point & First-Time Experience

The user installs the app on Arch Linux from AUR or a release package. On first launch, the app runs a setup check and shows dependency status for device communication, signing, background refresh, notifications, and permissions. If required services like `usbmuxd` are missing or stopped, the app provides exact commands the user can copy.

The empty state should say: “Connect your iPhone or iPad by USB to get started.” It should explain that USB is required for first-time pairing and that Wi-Fi refresh can be enabled after the device is trusted and reachable.

Core Experience

* Step 1: Initial setup check

  * Show a checklist of system requirements.
  * Validate `usbmuxd` service status, device permissions, installed signing backend, desktop notification support, and background agent status.
  * Provide fix actions where possible, such as “Start usbmuxd” or “Copy install command.”

* Step 2: Pair device over USB

  * User connects iPhone by USB.
  * App detects device and displays “Trust this computer on your iPhone.”
  * Once trusted, show device name, UDID, iOS version, battery level if available, and connection type.

* Step 3: Enable Wi-Fi refresh

  * App checks whether the paired device is discoverable over Wi-Fi.
  * If available, user clicks “Enable Wi-Fi refresh.”
  * App validates that the device can be reached without USB and stores the device as Wi-Fi refresh eligible.
  * If unavailable, show troubleshooting: same network, device unlocked, Wi-Fi sync/network pairing support, firewall rules, and reconnect by USB.

* Step 4: Import IPA

  * User drags an IPA onto the app or clicks “Choose IPA.”
  * App extracts metadata and shows app name, icon, bundle ID, version, and compatibility warnings.
  * User selects target device.

* Step 5: Sign app

  * User chooses signing method: free Apple ID, paid developer certificate/profile, or existing local signing assets.
  * For free Apple ID, explain 7-day expiration and possible app limits.
  * App signs the IPA locally and validates the output before installation.

* Step 6: Install app

  * User clicks “Install to Device.”
  * App shows progress by stage and streams concise logs behind an expandable details panel.
  * On success, app adds the installed app to the dashboard with expiration date and next scheduled refresh.

* Step 7: Automatic Wi-Fi refresh

  * Background agent checks installed apps daily.
  * If an app is within the refresh window and the device is reachable over Wi-Fi, the agent re-signs and reinstalls the app.
  * User receives a desktop notification on success or if action is required.

Edge Cases

* Device is connected but not trusted: Show trust prompt instructions and block installation until pairing succeeds.
* Device is locked: Ask the user to unlock the device and retry.
* Device not visible over Wi-Fi: Keep USB install available, show Wi-Fi troubleshooting, and mark automatic refresh as unavailable.
* Free Apple ID authentication fails: Show whether the issue is wrong credentials, 2FA required, rate-limiting, unsupported Apple authentication change, or unknown error when detectable.
* App expires before refresh: Mark the app as expired and guide the user through manual USB or Wi-Fi refresh.
* IPA has unsupported entitlements: Warn clearly and explain that some features may not work after signing.
* Bundle ID conflict: Offer to reuse the existing bundle ID when valid or generate a modified bundle ID.
* Apple service/API changes: Show a clear “signing service unavailable or changed” error and avoid pretending the issue is local.
* Background agent not running: Dashboard shows refresh automation disabled and offers a one-click setup or command instructions.
* Multiple devices with same app: Track expiration independently per device and app combination.

## Narrative

Maya uses an Arch-based Linux laptop as her daily machine and owns an iPhone she uses for personal utilities and test apps. Today, every time she wants to sideload an IPA, she has to borrow a Windows laptop or spin up a VM just to use a tool like Sideloadly. Even worse, because she uses a free Apple ID, the app stops working about a week later unless she remembers to refresh it manually.

She installs the Linux sideloading app from AUR, opens it, and sees a setup checklist. The app tells her `usbmuxd` is installed but not running, gives her the command to start it, and then detects her iPhone over USB. After she taps “Trust This Computer,” the app pairs with the device and confirms it can also see the phone over Wi-Fi.

Maya drags in an IPA, signs in with her Apple ID, and the app explains that the app will need to be refreshed every 6-7 days. She clicks Install, watches the progress complete, and sees the app appear on her phone. The dashboard now shows the expiration date and the next scheduled refresh. Six days later, while her phone and laptop are on the same Wi-Fi network, the background agent refreshes the app automatically and sends her a desktop notification. Maya does not have to think about the weekly resigning cycle anymore.

## Success Metrics

### User Metrics

* First sideload success rate: 90% or higher for supported Arch Linux environments using USB.
* Wi-Fi refresh success rate: 85% or higher when the device is paired, online, unlocked as needed, and on the same network.
* Setup completion rate: 80% or higher of new users complete dependency check, USB pairing, and first IPA import.
* Weekly active retained users: 50% or higher of users who sideload at least one app return within 14 days.
* User satisfaction: 4.3 out of 5 average rating from in-app feedback after successful install or refresh.

### Business Metrics

* Public release adoption: 500 active installs or 1,000 GitHub stars within 6 months.
* Support burden: Fewer than 20% of active users open troubleshooting issues after first setup.
* Community contribution: At least 5 external contributors or packaged distro maintainers within 6 months.

### Technical Metrics

* App launch time: Under 2 seconds on a typical Arch Linux desktop.
* Device detection time over USB: Under 5 seconds after connection in normal conditions.
* Wi-Fi device discovery time: Under 15 seconds on a local network.
* Signing failure classification: 95% of failures mapped to a specific user-actionable category.
* Background agent resource usage: Under 100 MB RAM idle and negligible CPU outside scheduled checks.
* Crash-free sessions: 99.5% or higher.

### Key Events to Track

* `setup_check_started`
* `setup_check_completed`
* `dependency_missing_detected`
* `device_usb_detected`
* `device_pairing_started`
* `device_pairing_completed`
* `device_wifi_detected`
* `wifi_refresh_enabled`
* `ipa_imported`
* `ipa_validation_failed`
* `signing_started`
* `signing_completed`
* `signing_failed`
* `install_started`
* `install_completed`
* `install_failed`
* `refresh_scheduled`
* `refresh_started`
* `refresh_completed`
* `refresh_failed`
* `notification_sent`
* `background_agent_enabled`
* `background_agent_disabled`

## Technical Considerations

* The app should only support user-owned devices and user-provided apps/signing credentials. It must not facilitate piracy, DRM bypass, or stolen certificate use.
* iOS signing with free Apple IDs is fragile because Apple authentication and provisioning flows can change. Build this layer as a replaceable adapter, not hardcoded throughout the app.
* USB and Wi-Fi device communication should use the `libimobiledevice` ecosystem where possible: `usbmuxd`, `libusbmuxd`, `libimobiledevice`, `idevice_id`, `ideviceinfo`, and `ideviceinstaller` or equivalent APIs.
* Wi-Fi refresh requires initial pairing and may depend on iOS settings, device state, local network conditions, firewall rules, and availability of network lockdown services.
* The product should start by shelling out to mature CLI tools behind a typed command runner, then move to direct Rust bindings only where reliability justifies the complexity.
* Signing should be implemented as a modular pipeline: unpack IPA, inspect metadata, prepare provisioning profile, adjust bundle ID/entitlements if needed, sign app bundle, repack IPA, validate signature.
* Sensitive data should be stored in the Linux Secret Service API when available, with a clear fallback for minimal environments.
* The background agent must be conservative: avoid constant network scanning, avoid waking the device excessively, and provide transparent logs.
* Apple may restrict free account signing limits, device limits, app counts, and authentication methods. The UI must set expectations clearly.

## UI Architecture

* Framework: Tauri 2 with React and TypeScript for the desktop UI, using Rust for device operations, signing orchestration, background jobs, and secure local storage integration.
* Component Library: shadcn/ui adapted for desktop-like workflows, with Radix primitives for accessible dialogs, dropdowns, tabs, and toasts.
* Styling: Tailwind CSS with a simple light/dark theme, system theme detection, and dense technical layouts for power users.
* State Management: TanStack Query for async device/signing/install operations and Zustand for local UI state such as selected device, selected IPA, filters, and wizard state.
* Animations: Minimal CSS transitions for progress, connection changes, and toast notifications. Avoid heavy animations that make the app feel less technical or trustworthy.

Responsive design should support laptop and desktop Linux displays first, with a minimum practical window width around 900 px. The app should remain usable at smaller widths but does not need a mobile layout.

Accessibility requirements include keyboard navigation for all core flows, visible focus states, screen-reader labels for status icons, non-color-only error indicators, and WCAG 2.1 AA contrast.

## API & Backend

* Framework: Tauri command backend in Rust with a local background agent. No cloud backend is required for MVP.
* Database: SQLite stored locally in the app data directory, accessed from Rust using `sqlx` or `rusqlite`.
* Authentication: No app-level account system. Apple ID authentication, when used, is handled locally through a signing provider adapter and stored only with user consent.
* Hosting: GitHub Releases for binaries, AUR for Arch packaging, optional Flatpak/AppImage distribution after MVP.
* Key API endpoints:
  * Local Tauri command `run_setup_check`: Checks dependencies, services, permissions, and background agent status.
  * Local Tauri command `list_devices`: Returns USB and Wi-Fi reachable iOS devices with connection status.
  * Local Tauri command `pair_device`: Starts or verifies device pairing over USB.
  * Local Tauri command `check_wifi_availability`: Tests whether a paired device can be reached over Wi-Fi.
  * Local Tauri command `import_ipa`: Extracts IPA metadata and validates structure.
  * Local Tauri command `sign_ipa`: Signs an imported IPA using selected signing method.
  * Local Tauri command `install_ipa`: Installs a signed IPA to the selected device.
  * Local Tauri command `list_installed_apps`: Shows apps managed by this tool and expiration metadata.
  * Local Tauri command `schedule_refresh`: Creates or updates refresh schedule for an installed app.
  * Local Tauri command `run_refresh_now`: Manually triggers refresh for selected app/device.
  * Local Tauri command `get_activity_log`: Returns recent operations and errors.

Suggested local SQLite tables:

* `devices`: UDID, name, iOS version, pairing status, Wi-Fi eligible flag, last seen timestamp.
* `apps`: app ID, display name, bundle ID, version, source IPA hash, icon path.
* `installations`: app ID, device UDID, signing method, install timestamp, expiration timestamp, refresh status.
* `signing_profiles`: signing method, team ID if available, profile metadata, encrypted local reference to secrets.
* `jobs`: scheduled refresh jobs, retry count, last run, next run, status.
* `activity_log`: timestamp, severity, operation, message, structured error code.

## Performance & Scalability

* Optimizations: Use lazy loading for logs and historical activity. Cache device metadata briefly but always refresh before install or signing-sensitive actions. Run signing and install operations off the UI thread. Stream progress events from Rust to React.
* Accessibility: Target WCAG 2.1 AA. Every error state must include text, not just color. Progress states must be announced in a screen-reader-friendly way where feasible.
* Scalability: MVP should support 1-5 devices and 1-20 managed apps per user. SQLite is sufficient. Avoid architectural complexity for multi-user or cloud sync scenarios.
* Monitoring: Use local structured logs with export capability. For public builds, optional privacy-preserving crash reporting can be offered as opt-in only. Include debug bundle export that redacts credentials and tokens.
* Reliability: Background refresh should use a systemd user service or desktop autostart entry. Jobs should be idempotent and safe to retry. Failed refreshes should not delete the currently installed app unless installation succeeds.
* Network behavior: Wi-Fi discovery should run on a schedule and on user request, not continuously. Use timeouts and clear error classification for unreachable devices.

## Integration Points

* libimobiledevice

  * What it does: Communicates with iOS devices for pairing, device info, lockdown services, and installation workflows.
  * SDK/library: Use system packages initially through CLI wrappers; evaluate Rust bindings later.
  * Configuration: Requires `usbmuxd` service, user permissions, and pairing records.

* usbmuxd/libusbmuxd

  * What it does: Handles USB multiplexing and communication with iOS devices over USB and, where supported, network connections.
  * SDK/library: System daemon and CLI integration.
  * Configuration: Must be installed, running, and accessible to the current user.

* ideviceinstaller or equivalent installation service

  * What it does: Installs signed IPA files onto iOS devices.
  * SDK/library: CLI wrapper for MVP.
  * Configuration: Requires paired/trusted device and valid signed IPA.

* zsign or equivalent signing tool

  * What it does: Signs IPA files with certificates and provisioning profiles.
  * SDK/library: Bundle or invoke as a managed binary where licensing permits, or use a Rust signing implementation later.
  * Configuration: Requires certificate, private key, provisioning profile, entitlements, and app bundle path.

* Linux Secret Service API

  * What it does: Stores Apple ID tokens, passwords, certificates, private key references, and other sensitive material locally.
  * SDK/library: Rust crates such as `secret-service` or `keyring`.
  * Configuration: Works with GNOME Keyring, KWallet, or compatible providers; provide fallback guidance for minimal window managers.

* systemd user services

  * What it does: Runs the background refresh agent and scheduled checks.
  * SDK/library: Generate and manage user service files from the app.
  * Configuration: Enable per-user service with clear UI status and manual command fallback.

* Desktop notifications

  * What it does: Sends refresh, expiration, and error notifications.
  * SDK/library: Tauri notification plugin or `notify-rust`.
  * Configuration: Requires notification daemon support in the user’s desktop environment.

* Apple authentication and provisioning services

  * What it does: Enables free Apple ID or developer account signing workflows where technically feasible.
  * SDK/library: Implement behind a provider adapter because APIs and authentication behavior can change.
  * Configuration: Must support 2FA-related flows where possible, avoid storing credentials unless the user opts in, and provide clear failure messages when Apple changes or blocks the flow.

* Analytics and crash reporting

  * What it does: Measures setup success, install success, refresh reliability, and crash rates.
  * SDK/library: Prefer local-only analytics for MVP. If external reporting is added, use opt-in Sentry or PostHog with strict redaction.
  * Configuration: Disable by default or ask during onboarding. Never send Apple ID, UDID, IPA names, bundle IDs, certificates, provisioning profiles, or logs containing secrets without explicit user review.