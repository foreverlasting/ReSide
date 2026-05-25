// Typed bridge to the Rust backend. Every Tauri command has a wrapper here so
// screens never call `invoke` with stringly-typed names. Types mirror the
// `#[derive(Serialize)]` structs in src-tauri/src/lib.rs and reside-core.

import { invoke } from "@tauri-apps/api/core";

/** True when running inside the Tauri webview (vs. a plain browser via `pnpm dev`). */
export const isTauri = (): boolean =>
  typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** UI-safe error shape returned on the `Err` path of every command. */
export interface CommandError {
  category: string;
  remediation: string;
}

/** Narrow an unknown thrown value to a CommandError when possible. */
export function asCommandError(e: unknown): CommandError {
  if (e && typeof e === "object" && "category" in e && "remediation" in e) {
    return e as CommandError;
  }
  return { category: "Internal", remediation: String(e) };
}

export type CheckStatus = "ok" | "warn";

export interface SetupCheck {
  key: string;
  label: string;
  status: CheckStatus;
  detail: string;
}

export interface SetupReport {
  items: SetupCheck[];
  ok: number;
  warn: number;
}

/** Aggregate titlebar pill state (not device-scoped). */
export interface TunnelPill {
  connected: boolean;
}

export interface TunnelEndpoint {
  serverAddress: string;
  clientAddress: string;
  rsdPort: number;
  mtu: number;
}

export interface DiscoveredService {
  name: string;
  port: number;
}

/** Per-device tunnel status returned by `establish_tunnel`. */
export interface TunnelStatus {
  udid: string;
  connected: boolean;
  endpoint: TunnelEndpoint | null;
  services: DiscoveredService[];
}

export interface ActivityRow {
  ts: number;
  severity: string;
  operation: string | null;
  error_category: string | null;
  message: string | null;
}

/** One RemoteXPC-capable endpoint found on the local network via mDNS. */
export interface WifiEndpoint {
  serviceType: string;
  host: string;
  addresses: string[];
  port: number;
}

/** Result of a Wi-Fi reachability scan (not yet device-scoped). */
export interface WifiAvailability {
  available: boolean;
  endpoints: WifiEndpoint[];
}

export interface DeviceInfo {
  udid: string;
  name: string | null;
  iosVersion: string | null;
  productType: string | null;
  connection: string;
  wifi: boolean;
  supported: boolean;
}

/** Returned by `install_ipa` on success. */
export interface InstallOutcome {
  installationId: number;
  displayName: string;
  bundleId: string;
  expirationTs: number;
}

/** One installed app (installation row joined with its app metadata). */
export interface InstalledApp {
  installationId: number;
  displayName: string;
  bundleId: string;
  version: string | null;
  deviceUdid: string;
  installTs: number;
  expirationTs: number;
  refreshStatus: string;
}

/** Arguments for the `install_ipa` command. `twoFaCode` is set only on a retry
 *  after a `AppleAuth2FARequired` error. */
export interface InstallArgs {
  operationId: string;
  path: string;
  udid: string;
  twoFaCode?: string;
}

/** Returned by `refresh_app` on a successful single-app refresh. */
export interface RefreshAppOutcome {
  installationId: number;
  newExpirationTs: number;
}

/** Per-install result inside a batch refresh. Field names mirror the Rust
 *  `RefreshOutcome` (enum tag `result`; inner fields stay snake_case). */
export type RefreshOutcome =
  | { result: "refreshed"; new_expiration_ts: number }
  | { result: "retrying"; category: string; next_run: number }
  | { result: "needs_attention"; category: string };

export interface RefreshReport {
  installationId: number;
  bundleId: string;
  displayName: string;
  outcome: RefreshOutcome;
}

/** Result of a `refresh_due_now` batch. `ran` is false when another process
 *  held the single-writer lock and we declined to run concurrently. */
export interface RefreshSummary {
  ran: boolean;
  attempted: number;
  refreshed: number;
  reports: RefreshReport[];
}

/** Arguments for `refresh_app`. */
export interface RefreshAppArgs {
  operationId: string;
  installationId: number;
}

/** State of the background-refresh autopilot (mirrors Rust `AgentStatus`). */
export interface AgentStatus {
  enabled: boolean;
  mechanism: "systemd" | "xdg_autostart";
  detail: string;
}

export const api = {
  runSetupCheck: () => invoke<SetupReport>("run_setup_check"),
  getTunnelStatus: () => invoke<TunnelPill>("get_tunnel_status"),
  establishTunnel: (udid: string) => invoke<TunnelStatus>("establish_tunnel", { udid }),
  listDevices: () => invoke<DeviceInfo[]>("list_devices"),
  pairDevice: (udid: string) => invoke<void>("pair_device", { udid }),
  developerModeStatus: (udid: string) => invoke<boolean>("developer_mode_status", { udid }),
  checkWifiAvailability: () => invoke<WifiAvailability>("check_wifi_availability"),
  getActivityLog: () => invoke<ActivityRow[]>("get_activity_log"),
  // Sign / install (task 11b).
  pickIpa: () => invoke<string | null>("pick_ipa"),
  isSignedIn: () => invoke<boolean>("is_signed_in"),
  setAppleCredentials: (appleId: string, password: string) =>
    invoke<void>("set_apple_credentials", { appleId, password }),
  signOut: () => invoke<void>("sign_out"),
  installIpa: ({ operationId, path, udid, twoFaCode }: InstallArgs) =>
    invoke<InstallOutcome>("install_ipa", { operationId, path, udid, twoFaCode }),
  listApps: () => invoke<InstalledApp[]>("list_apps"),
  // Auto-refresh (task 11c).
  refreshApp: ({ operationId, installationId }: RefreshAppArgs) =>
    invoke<RefreshAppOutcome>("refresh_app", { operationId, installationId }),
  refreshDueNow: () => invoke<RefreshSummary>("refresh_due_now"),
  // Background autopilot (task 11c slice 2).
  agentStatus: () => invoke<AgentStatus>("agent_status"),
  setBackgroundAgent: (enabled: boolean) =>
    invoke<AgentStatus>("set_background_agent", { enabled }),
};
