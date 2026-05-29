// The live application shell (runs inside Tauri). The app ALWAYS opens to the
// Dashboard — there is no gated Setup→Pairing wizard. First-time guidance lives
// in the dashboard's inline "Get Started" panel (driven by `getStarted`); the
// Setup ("System") and Pairing screens are pushed as on-demand overlays from
// that panel, the sidebar, or Settings. Live status (backend/tunnel + theme
// toggle) is injected into the titlebar; detected devices into the sidebar.

import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Settings } from "./screens/Settings";
import { Activity } from "./screens/Activity";
import { Dashboard } from "./screens/Dashboard";
import { ImportModal } from "./screens/ImportModal";
import { RefreshModal } from "./screens/RefreshModal";
import {
  Pairing,
  type DevModeState,
  type PairPhase,
  type TunnelPhase,
  type WifiPhase,
} from "./screens/Pairing";
import {
  api,
  asCommandError,
  isTauri,
  type CommandError,
  type DeviceInfo,
  type InstalledApp,
} from "./lib/ipc";
import { Icon } from "./components/ui";
import { cn } from "./lib/cn";

// On-demand surfaces layered over the always-present dashboard. The detailed
// "system" view is intentionally absent: the Dashboard's inline system check is
// the single source for that (ROADMAP §7d).
type Overlay = "pairing" | "settings" | "activity" | null;

const THEME_KEY = "reside-theme";

export function ReSideApp() {
  // Persisted across launches; first run falls back to the OS preference.
  const [dark, setDark] = useState(() => {
    try {
      const saved = localStorage.getItem(THEME_KEY);
      if (saved) return saved === "dark";
      return window.matchMedia?.("(prefers-color-scheme: dark)").matches ?? false;
    } catch {
      return false;
    }
  });
  useEffect(() => {
    try {
      localStorage.setItem(THEME_KEY, dark ? "dark" : "light");
    } catch {
      /* storage unavailable — theme just won't persist this session */
    }
  }, [dark]);
  const [overlay, setOverlay] = useState<Overlay>(null);
  const [systemCheckExpanded, setSystemCheckExpanded] = useState(false);
  const [selectedUdid, setSelectedUdid] = useState<string | null>(null);
  const [importing, setImporting] = useState(false);
  const [refreshTarget, setRefreshTarget] = useState<InstalledApp | null>(null);
  const queryClient = useQueryClient();

  const setup = useQuery({ queryKey: ["setup-check"], queryFn: api.runSetupCheck });
  const tunnel = useQuery({ queryKey: ["tunnel-status"], queryFn: api.getTunnelStatus });
  // Poll so a device appears within ~2s of being plugged in.
  const devices = useQuery({
    queryKey: ["devices"],
    queryFn: api.listDevices,
    refetchInterval: 2000,
  });

  // Installed apps for the live Dashboard grid.
  const apps = useQuery({ queryKey: ["apps"], queryFn: api.listApps, enabled: isTauri() });

  // Where Apple credentials are held. Auto-refresh needs them persisted (keyring),
  // since the unattended agent has no UI to prompt; session-only creds can't.
  const credStatus = useQuery({
    queryKey: ["cred-status"],
    queryFn: api.credentialStatus,
    enabled: isTauri(),
  });
  const canEnableAgent = credStatus.data?.mode === "keyring";

  const deviceList = useMemo(() => devices.data ?? [], [devices.data]);
  // The device we'll pair: the explicitly-selected one, else the first seen.
  const target = useMemo(
    () => deviceList.find((d) => d.udid === selectedUdid) ?? deviceList[0],
    [deviceList, selectedUdid]
  );

  const pair = useMutation({
    mutationFn: (udid: string) => api.pairDevice(udid),
    onSuccess: () => devices.refetch(),
  });

  // "Refresh all due" — the same due-check the background agent runs on a timer.
  const refreshAll = useMutation({
    mutationFn: () => api.refreshDueNow(),
    onSettled: () => apps.refetch(),
  });

  // Background autopilot: whether the systemd timer / autostart trigger is
  // installed, plus the toggle that installs/removes it.
  const agent = useQuery({ queryKey: ["agent-status"], queryFn: api.agentStatus, enabled: isTauri() });
  const setAgent = useMutation({
    mutationFn: (enabled: boolean) => api.setBackgroundAgent(enabled),
    onSuccess: () => agent.refetch(),
  });

  const phase: PairPhase = pair.isPending
    ? "pairing"
    : pair.isSuccess
      ? "paired"
      : pair.isError
        ? "error"
        : "idle";

  // Once paired, read Developer Mode over the trusted amfi service. iOS 17.4+
  // requires it for install flows, so we gate on it right after pairing.
  const devMode = useQuery({
    queryKey: ["dev-mode", target?.udid],
    queryFn: () => api.developerModeStatus(target!.udid),
    enabled: phase === "paired" && !!target,
  });

  const developerMode: DevModeState =
    phase !== "paired"
      ? "idle"
      : devMode.isError
        ? "unknown"
        : devMode.data === true
          ? "on"
          : devMode.data === false
            ? "off"
            : "checking";

  // Once Developer Mode is confirmed on, the device is ready for the RSD tunnel
  // — the gateway to install + Wi-Fi refresh. Establish it on demand.
  const tunnelEstablish = useMutation({
    mutationFn: (udid: string) => api.establishTunnel(udid),
    onSuccess: () => tunnel.refetch(),
  });

  const tunnelPhase: TunnelPhase = tunnelEstablish.isPending
    ? "connecting"
    : tunnelEstablish.isSuccess
      ? "connected"
      : tunnelEstablish.isError
        ? "error"
        : "idle";

  // Wi-Fi reachability: an on-demand mDNS scan for RemoteXPC endpoints. Not
  // device-scoped yet — it answers "is any iOS device reachable on this network?"
  const wifiCheck = useMutation({
    mutationFn: () => api.checkWifiAvailability(),
  });

  // A passive Wi-Fi-reachability ping for the Dashboard rail. Runs ONLY when
  // the USB list is empty (so we don't bother the network when a cable is in)
  // and re-checks every 30s so a phone that joins the LAN becomes visible
  // without the user clicking anything. Independent from the manual `wifiCheck`
  // the Pairing screen uses.
  const wifiRailCheck = useQuery({
    queryKey: ["wifi-rail-check"],
    queryFn: api.checkWifiAvailability,
    enabled: isTauri() && !devices.isLoading && deviceList.length === 0,
    refetchInterval: 30_000,
    staleTime: 25_000,
  });

  // "Connect over Wi-Fi": spin netmuxd up, resolve the full named card, cache
  // it for the session, tear netmuxd down. Cold path is ~38s.
  const resolveWifi = useMutation({
    mutationFn: () => api.resolveWifiDevices(),
    onSuccess: () => {
      // The cache merge happens in `list_devices`; pull a fresh copy so the
      // rail picks the new card up.
      queryClient.invalidateQueries({ queryKey: ["devices"] });
    },
  });

  const wifiPhase: WifiPhase = wifiCheck.isPending
    ? "checking"
    : wifiCheck.isSuccess
      ? "done"
      : wifiCheck.isError
        ? "error"
        : "idle";

  const setupError = setup.error ? asCommandError(setup.error) : null;
  const backendTone = setupError ? "danger" : setup.isLoading ? "warning" : "success";
  const backendText = setupError
    ? setupError.category
    : setup.isLoading
      ? "Connecting…"
      : "Backend OK";

  // System dependencies are "ready" once the check has returned with no
  // warnings. Drives the Get Started panel's first step + onboarding stage.
  const systemReady = !!setup.data && setup.data.warn === 0;

  const toolbarExtra = (
    <div className="flex items-center gap-1.5">
      <StatusPill tone={backendTone} label={backendText} />
      <StatusPill
        tone={tunnel.data?.connected ? "success" : "neutral"}
        label={tunnel.data?.connected ? "Tunnel" : "No tunnel"}
      />
      <button
        onClick={() => setDark((v) => !v)}
        aria-label="Toggle theme"
        className="flex h-7 w-7 items-center justify-center rounded-md text-slate-500 hover:bg-slate-200/60 hover:text-slate-900 dark:hover:bg-slate-800 dark:hover:text-slate-100"
      >
        <Icon name={dark ? "moon" : "sun"} size={14} />
      </button>
    </div>
  );

  const closeOverlay = () => {
    pair.reset();
    tunnelEstablish.reset();
    wifiCheck.reset();
    setOverlay(null);
  };

  return (
    <div
      className="h-screen w-screen overflow-hidden"
      style={{ background: dark ? "#21222c" : "#dce0e8" }}
    >
      {overlay === "settings" ? (
        <Settings
          dark={dark}
          onClose={closeOverlay}
          toolbarExtra={toolbarExtra}
          railExtra={
            <DevicesRail
              devices={deviceList}
              error={devices.error}
              selectedUdid={target?.udid}
              onSelect={setSelectedUdid}
              wifiReachable={wifiRailCheck.data?.available ?? false}
              wifiChecking={wifiRailCheck.isFetching}
              resolving={resolveWifi.isPending}
              resolveError={resolveWifi.error ? asCommandError(resolveWifi.error) : null}
              onConnectWifi={() => resolveWifi.mutate()}
              onRescanWifi={() => wifiRailCheck.refetch()}
            />
          }
        />
      ) : overlay === "activity" ? (
        <Activity dark={dark} onClose={closeOverlay} toolbarExtra={toolbarExtra} />
      ) : overlay === "pairing" ? (
        <Pairing
          dark={dark}
          device={target}
          phase={phase}
          error={pair.error ? asCommandError(pair.error) : null}
          developerMode={developerMode}
          tunnelPhase={tunnelPhase}
          tunnelStatus={tunnelEstablish.data}
          tunnelError={tunnelEstablish.error ? asCommandError(tunnelEstablish.error) : null}
          wifiPhase={wifiPhase}
          wifiAvailability={wifiCheck.data}
          wifiError={wifiCheck.error ? asCommandError(wifiCheck.error) : null}
          onEstablishTunnel={() => target && tunnelEstablish.mutate(target.udid)}
          onCheckWifi={() => wifiCheck.mutate()}
          onPair={() => target && pair.mutate(target.udid)}
          onRecheckDevMode={() => devMode.refetch()}
          onSkip={closeOverlay}
          onContinue={closeOverlay}
          onBack={closeOverlay}
        />
      ) : (
        <>
          <Dashboard
            dark={dark}
            live
            device={target ?? null}
            apps={apps.data ?? []}
            toolbarExtra={toolbarExtra}
            onImport={target ? () => setImporting(true) : undefined}
            onRefreshApp={(app) => setRefreshTarget(app)}
            onRefreshAll={() => refreshAll.mutate()}
            refreshingAll={refreshAll.isPending}
            sidebarNoDeviceFallback={
              <WifiEmptyState
                wifiReachable={wifiRailCheck.data?.available ?? false}
                wifiChecking={wifiRailCheck.isFetching}
                resolving={resolveWifi.isPending}
                resolveError={resolveWifi.error ? asCommandError(resolveWifi.error) : null}
                onConnectWifi={() => resolveWifi.mutate()}
                onRescanWifi={() => wifiRailCheck.refetch()}
              />
            }
            agentEnabled={agent.data?.enabled ?? false}
            agentDetail={agent.data?.detail}
            agentBusy={setAgent.isPending}
            agentError={setAgent.error ? asCommandError(setAgent.error).remediation : null}
            onToggleAgent={(enabled) => setAgent.mutate(enabled)}
            getStarted={{
              systemReady,
              report: setup.data,
              expanded: systemCheckExpanded,
              rerunning: setup.isFetching,
              onRunCheck: () => {
                setSystemCheckExpanded(true);
                setup.refetch();
              },
              onPair: () => setOverlay("pairing"),
              onEnableAgent: () => setAgent.mutate(true),
              agentBusy: setAgent.isPending,
              canEnableAgent,
            }}
            onNavigate={(id) => {
              if (id === "apps") setOverlay(null);
              else if (id === "devices") setOverlay("pairing");
              else if (id === "activity") setOverlay("activity");
              else if (id === "settings") setOverlay("settings");
            }}
          />
          {importing && (
            <ImportModal
              device={target ?? null}
              onClose={() => {
                setImporting(false);
                // A session/ask sign-in may have changed where creds live.
                credStatus.refetch();
              }}
              onInstalled={() => {
                setImporting(false);
                apps.refetch();
                // Reflect a fresh keyring sign-in so auto-refresh can be enabled.
                credStatus.refetch();
              }}
              onManageCerts={() => {
                setImporting(false);
                setOverlay("settings");
              }}
            />
          )}
          {refreshTarget && (
            <RefreshModal
              app={refreshTarget}
              onClose={() => setRefreshTarget(null)}
              onRefreshed={() => apps.refetch()}
              onManageCerts={() => {
                setRefreshTarget(null);
                setOverlay("settings");
              }}
            />
          )}
        </>
      )}
    </div>
  );
}

function StatusPill({
  tone,
  label,
}: {
  tone: "success" | "warning" | "danger" | "neutral";
  label: string;
}) {
  const dot = {
    success: "bg-emerald-500",
    warning: "bg-amber-500",
    danger: "bg-red-500",
    neutral: "bg-slate-400",
  }[tone];
  return (
    <span className="flex items-center gap-1.5 rounded-full border border-slate-200 bg-white px-2 py-0.5 text-[11px] text-slate-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
      <span className={cn("inline-flex h-1.5 w-1.5 rounded-full", dot)} />
      {label}
    </span>
  );
}

function DevicesRail({
  devices,
  error,
  selectedUdid,
  onSelect,
  wifiReachable = false,
  wifiChecking = false,
  resolving = false,
  resolveError = null,
  onConnectWifi,
  onRescanWifi,
}: {
  devices: DeviceInfo[];
  error: unknown;
  selectedUdid?: string;
  onSelect: (udid: string) => void;
  /** mDNS just spotted an iOS device on the LAN — we don't know its name yet. */
  wifiReachable?: boolean;
  /** A passive reachability check is in flight (the soft 3s mDNS poll). */
  wifiChecking?: boolean;
  /** "Connect over Wi-Fi" is currently spinning netmuxd up + waiting on discovery. */
  resolving?: boolean;
  resolveError?: CommandError | null;
  /** Click handler for the "Connect over Wi-Fi" button. */
  onConnectWifi?: () => void;
  /** Optional manual rescan of the mDNS reachability check. */
  onRescanWifi?: () => void;
}) {
  return (
    <div className="rounded-md border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
      <div className="mb-2 flex items-center gap-1.5 text-[10.5px] font-semibold uppercase tracking-wider text-slate-500">
        <Icon name="smartphone" size={12} />
        Devices
        <span className="ml-auto font-normal normal-case text-slate-400">
          {error ? "usbmuxd?" : `${devices.length}`}
        </span>
      </div>
      {devices.length === 0 ? (
        <WifiEmptyState
          error={error}
          wifiReachable={wifiReachable}
          wifiChecking={wifiChecking}
          resolving={resolving}
          resolveError={resolveError}
          onConnectWifi={onConnectWifi}
          onRescanWifi={onRescanWifi}
        />
      ) : (
        <div className="space-y-1.5">
          {devices.map((d) => (
            <button
              key={d.udid}
              onClick={() => onSelect(d.udid)}
              className={cn(
                "flex w-full items-center gap-2 rounded px-1.5 py-1 text-left transition-colors",
                d.udid === selectedUdid
                  ? "bg-slate-100 dark:bg-slate-800"
                  : "hover:bg-slate-50 dark:hover:bg-slate-800/60"
              )}
            >
              <span
                className={cn(
                  "inline-flex h-2 w-2 shrink-0 rounded-full",
                  d.supported ? "bg-emerald-500" : "bg-red-500"
                )}
              />
              <span className="truncate text-[12px] font-medium text-slate-800 dark:text-slate-200">
                {d.name ?? `${d.udid.slice(0, 8)}…`}
              </span>
              <span className="ml-auto flex shrink-0 items-center gap-1 text-[10px] text-slate-500">
                {d.iosVersion && <span>iOS {d.iosVersion}</span>}
                <span className="rounded border border-slate-300 px-1 dark:border-slate-700">
                  {d.wifi ? "Wi-Fi" : d.connection}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

/// Empty-state body for any device list (DevicesRail and the Sidebar's
/// Devices card share this). Three branches:
///
/// 1. The mDNS reachability ping found *something* on the LAN → soft banner
///    plus a "Connect over Wi-Fi" button that drives the resolve mutation.
/// 2. The resolve is in flight → spinner + "Locating your iPhone over
///    Wi-Fi… (~40s)" so the long cold-discovery wait isn't a silent void.
/// 3. Nothing reachable → the original "Plug in your iPhone over USB." hint.
export function WifiEmptyState({
  error,
  wifiReachable,
  wifiChecking,
  resolving,
  resolveError,
  onConnectWifi,
  onRescanWifi,
}: {
  error?: unknown;
  wifiReachable: boolean;
  wifiChecking: boolean;
  resolving: boolean;
  resolveError: CommandError | null;
  onConnectWifi?: () => void;
  onRescanWifi?: () => void;
}) {
  if (error) {
    return (
      <div className="text-[11.5px] text-slate-500">{asCommandError(error).remediation}</div>
    );
  }

  if (resolving) {
    return (
      <div className="space-y-1.5">
        <div className="flex items-center gap-1.5 text-[11.5px] text-slate-600 dark:text-slate-300">
          <span className="inline-block h-3 w-3 animate-spin rounded-full border-2 border-slate-300 border-t-slate-600 dark:border-slate-700 dark:border-t-slate-200" />
          Locating your iPhone over Wi-Fi…
        </div>
        <div className="text-[10.5px] text-slate-500">
          First connection takes about 40 seconds.
        </div>
      </div>
    );
  }

  if (wifiReachable) {
    return (
      <div className="space-y-2">
        <div className="text-[11.5px] text-slate-600 dark:text-slate-300">
          An iPhone is reachable over Wi-Fi.
        </div>
        <div className="flex flex-wrap items-center gap-1.5">
          <button
            onClick={onConnectWifi}
            disabled={!onConnectWifi}
            className="inline-flex items-center gap-1.5 rounded-md border border-slate-300 bg-white px-2 py-1 text-[11px] font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-200 dark:hover:bg-slate-800"
          >
            <Icon name="wifi" size={11} />
            Connect over Wi-Fi
          </button>
          {onRescanWifi && (
            <button
              onClick={onRescanWifi}
              className="rounded-md px-1.5 py-1 text-[10.5px] text-slate-500 hover:bg-slate-100 dark:hover:bg-slate-800"
            >
              Rescan
            </button>
          )}
        </div>
        {resolveError && (
          <div className="text-[10.5px] text-red-500">{resolveError.remediation}</div>
        )}
      </div>
    );
  }

  return (
    <div className="text-[11.5px] text-slate-500">
      {wifiChecking ? "Looking for iPhones on Wi-Fi…" : "Plug in your iPhone over USB."}
    </div>
  );
}
