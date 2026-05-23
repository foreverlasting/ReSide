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

export const api = {
  runSetupCheck: () => invoke<SetupReport>("run_setup_check"),
  getTunnelStatus: () => invoke<TunnelPill>("get_tunnel_status"),
  establishTunnel: (udid: string) => invoke<TunnelStatus>("establish_tunnel", { udid }),
  listDevices: () => invoke<DeviceInfo[]>("list_devices"),
  pairDevice: (udid: string) => invoke<void>("pair_device", { udid }),
  developerModeStatus: (udid: string) => invoke<boolean>("developer_mode_status", { udid }),
  checkWifiAvailability: () => invoke<WifiAvailability>("check_wifi_availability"),
  getActivityLog: () => invoke<ActivityRow[]>("get_activity_log"),
};
