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
import { System } from "./screens/System";
import { Dashboard, type GetStartedHandlers } from "./screens/Dashboard";
import { ImportModal } from "./screens/ImportModal";
import { RefreshModal } from "./screens/RefreshModal";
import { Devices } from "./screens/Devices";
import { PairModal } from "./screens/PairModal";
import type {
  DevModeState,
  PairPhase,
  TunnelPhase,
  WifiPhase,
} from "./screens/Pairing";
import {
  api,
  asCommandError,
  isTauri,
  type CommandError,
  type InstalledApp,
} from "./lib/ipc";
import { Icon } from "./components/ui";
import { cn } from "./lib/cn";

// The main pane shown inside the persistent shell. The sidebar, window chrome,
// and toolbar stay put across all of these — only this pane swaps (ROADMAP §7h).
// "devices" is now a first-class pane too (ROADMAP §7e/§7f): the old full-screen
// Pairing takeover is gone. The only transient overlay left is the focused trust
// handshake (`PairModal`, `pairModalOpen`), opened from the Devices pane.
type Surface = "apps" | "devices" | "activity" | "settings" | "system";

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
  const [surface, setSurface] = useState<Surface>("apps");
  const [pairModalOpen, setPairModalOpen] = useState(false);
  // Which device the panes act on. Defaults to the first detected; the Devices
  // switcher lets the user pick another when several are plugged in.
  const [selectedUdid, setSelectedUdid] = useState<string | null>(null);
  const [systemCheckExpanded, setSystemCheckExpanded] = useState(false);
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

  // Has a device ever been paired? Wi-Fi refresh rides on the USB-minted pairing
  // record, so the "Connect over Wi-Fi" affordance is gated on this (ROADMAP §7i).
  // A successful install is the persistent proof of a pairing (it writes the
  // device's `pairing_status='paired'` row); `pair.isSuccess` covers the just-
  // -paired-this-session case before any install exists.
  const hasInstalls = (apps.data?.length ?? 0) > 0;

  const deviceList = useMemo(() => devices.data ?? [], [devices.data]);
  // The device the panes act on: the user's selection in the Devices switcher,
  // falling back to the first detected one (ROADMAP §7f). All the per-device
  // queries below key off this, so changing the selection re-scopes them.
  const target = useMemo(
    () => deviceList.find((d) => d.udid === selectedUdid) ?? deviceList[0],
    [deviceList, selectedUdid]
  );

  const pair = useMutation({
    mutationFn: (udid: string) => api.pairDevice(udid),
    onSuccess: () => devices.refetch(),
  });

  // Combine the persistent (install record) and session (just-paired) signals.
  const hasPairedDevice = hasInstalls || pair.isSuccess;

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

  // Read Developer Mode over the trusted amfi service. iOS 17.4+ requires it for
  // install flows. Gated on the *standing* paired state (an install record, or a
  // just-completed pair) rather than the transient pair phase, so the Devices
  // ladder shows it whenever there's a paired device — not only right after the
  // handshake. Re-keys (and refetches) when the switcher changes `target`.
  const devMode = useQuery({
    queryKey: ["dev-mode", target?.udid],
    queryFn: () => api.developerModeStatus(target!.udid),
    enabled: isTauri() && !!target && hasPairedDevice,
  });

  const developerMode: DevModeState = !hasPairedDevice
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

  // The trust modal owns only the handshake; the tunnel/Wi-Fi mutations now live
  // in the Devices ladder, so opening/closing the modal must NOT reset them.
  // Each open starts the handshake fresh; close just hides it, preserving
  // `pair.isSuccess` so the ladder's "Paired" rung stays lit afterwards.
  const openPairModal = () => {
    pair.reset();
    setPairModalOpen(true);
    setSurface("devices");
  };
  const closePairModal = () => setPairModalOpen(false);

  // Pairing is USB-only; a Wi-Fi-resolved card can't mint a pairing record.
  const canPair = !!target && !target.wifi;

  // The onboarding/system-check handlers, shared by the Dashboard's inline "Get
  // Started" panel and the standing System pane (ROADMAP §7h/§7d).
  const getStarted: GetStartedHandlers = {
    systemReady,
    report: setup.data,
    expanded: systemCheckExpanded,
    rerunning: setup.isFetching,
    onRunCheck: () => {
      setSystemCheckExpanded(true);
      setup.refetch();
    },
    onPair: openPairModal,
    onEnableAgent: () => setAgent.mutate(true),
    agentBusy: setAgent.isPending,
    canEnableAgent,
  };

  return (
    // `data-theme` is hoisted to the app root (not just each GnomeWindow) so it
    // anchors the dark-variant *descendant* selectors for EVERY surface,
    // including the install/refresh modals that render as siblings of the
    // Dashboard — outside any GnomeWindow wrapper. Without this they kept their
    // light values in dark mode (ROADMAP §7j). The per-window `data-theme` in
    // GnomeWindow is now redundant but harmless (same value, same selector).
    <div
      data-theme={dark ? "dark" : "light"}
      className="h-screen w-screen overflow-hidden"
      style={{ background: dark ? "#21222c" : "#dce0e8" }}
    >
      {/* ONE Dashboard — and one persistent sidebar — stays mounted across every
          surface; `mainContent` swaps only the right-hand pane, and `active`
          moves the nav highlight (ROADMAP §7h). The trust handshake is the only
          overlay now — a sibling modal below, not a takeover (ROADMAP §7e/§7f). */}
      <Dashboard
        dark={dark}
        live
        active={surface}
        subtitleOverride={
          surface === "settings"
            ? "Settings"
            : surface === "devices"
              ? "Devices"
              : surface === "activity"
                ? "Activity"
                : surface === "system"
                  ? "System"
                  : undefined
        }
        mainContent={
          surface === "settings" ? (
            <Settings />
          ) : surface === "devices" ? (
            <Devices
              devices={deviceList}
              selectedUdid={target?.udid ?? null}
              onSelect={setSelectedUdid}
              paired={hasPairedDevice}
              developerMode={developerMode}
              tunnelPhase={tunnelPhase}
              tunnelStatus={tunnelEstablish.data}
              tunnelError={tunnelEstablish.error ? asCommandError(tunnelEstablish.error) : null}
              wifiPhase={wifiPhase}
              wifiAvailability={wifiCheck.data}
              wifiError={wifiCheck.error ? asCommandError(wifiCheck.error) : null}
              onPair={canPair ? openPairModal : undefined}
              onRecheckDevMode={() => devMode.refetch()}
              onEstablishTunnel={() => target && tunnelEstablish.mutate(target.udid)}
              onCheckWifi={() => wifiCheck.mutate()}
              wifiReachable={wifiRailCheck.data?.available ?? false}
              wifiChecking={wifiRailCheck.isFetching}
              resolving={resolveWifi.isPending}
              resolveError={resolveWifi.error ? asCommandError(resolveWifi.error) : null}
              onConnectWifi={hasPairedDevice ? () => resolveWifi.mutate() : undefined}
              onRescanWifi={() => wifiRailCheck.refetch()}
            />
          ) : surface === "activity" ? (
            <Activity />
          ) : surface === "system" ? (
            <System gs={getStarted} />
          ) : undefined
        }
        device={target ?? null}
        apps={apps.data ?? []}
        toolbarExtra={toolbarExtra}
        onImport={target ? () => setImporting(true) : undefined}
        onRefreshApp={(app) => setRefreshTarget(app)}
        onRefreshAll={() => refreshAll.mutate()}
        refreshingAll={refreshAll.isPending}
        sidebarNoDeviceFallback={
          <WifiEmptyState
            paired={hasPairedDevice}
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
        getStarted={getStarted}
        onNavigate={(id) => {
          if (
            id === "apps" ||
            id === "devices" ||
            id === "activity" ||
            id === "settings" ||
            id === "system"
          )
            setSurface(id);
        }}
      />
      {pairModalOpen && (
        <PairModal
          device={target ?? null}
          phase={phase}
          error={pair.error ? asCommandError(pair.error) : null}
          onPair={() => target && pair.mutate(target.udid)}
          onClose={closePairModal}
        />
      )}
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
            setSurface("settings");
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
            setSurface("settings");
          }}
        />
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

/// Empty-state body for the Sidebar's Devices card. Three branches:
///
/// 1. The mDNS reachability ping found *something* on the LAN → soft banner
///    plus a "Connect over Wi-Fi" button that drives the resolve mutation.
/// 2. The resolve is in flight → spinner + "Locating your iPhone over
///    Wi-Fi… (~40s)" so the long cold-discovery wait isn't a silent void.
/// 3. Nothing reachable → the original "Plug in your iPhone over USB." hint.
export function WifiEmptyState({
  error,
  paired = true,
  wifiReachable,
  wifiChecking,
  resolving,
  resolveError,
  onConnectWifi,
  onRescanWifi,
}: {
  error?: unknown;
  /** Whether a device has ever been paired (an install record exists). Wi-Fi
   *  refresh rides on the USB-minted pairing record, so "Connect over Wi-Fi" is
   *  offered ONLY once paired; before that we nudge the user to pair over USB
   *  first instead of dangling an action that can't work yet (ROADMAP §7i). */
  paired?: boolean;
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

  // Reachable on Wi-Fi but never paired: connecting/refreshing needs a pairing
  // record that only a USB pair creates, so point there rather than offer Connect.
  if (wifiReachable && !paired) {
    return (
      <div className="space-y-1.5">
        <div className="text-[11.5px] text-slate-600 dark:text-slate-300">
          An iPhone is on Wi-Fi.
        </div>
        <div className="text-[10.5px] text-slate-500">
          Plug it in over USB once to pair — then Wi-Fi refresh works on its own.
        </div>
      </div>
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
