// The live application shell (runs inside Tauri). It owns a small screen state
// machine (Setup → Pairing) and the device-pairing mutation. Live status
// (backend/tunnel + theme toggle) is injected into the window titlebar and
// detected devices into the sidebar rail — no floating overlays on content.

import { useMemo, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Setup } from "./screens/Setup";
import { Dashboard } from "./screens/Dashboard";
import {
  Pairing,
  type DevModeState,
  type PairPhase,
  type TunnelPhase,
  type WifiPhase,
} from "./screens/Pairing";
import { api, asCommandError, type DeviceInfo } from "./lib/ipc";
import { Icon } from "./components/ui";
import { cn } from "./lib/cn";

type Screen = "setup" | "pairing" | "dashboard";

export function ReSideApp() {
  const [dark, setDark] = useState(false);
  const [screen, setScreen] = useState<Screen>("setup");
  const [selectedUdid, setSelectedUdid] = useState<string | null>(null);

  const setup = useQuery({ queryKey: ["setup-check"], queryFn: api.runSetupCheck });
  const tunnel = useQuery({ queryKey: ["tunnel-status"], queryFn: api.getTunnelStatus });
  // Poll so a device appears within ~2s of being plugged in.
  const devices = useQuery({
    queryKey: ["devices"],
    queryFn: api.listDevices,
    refetchInterval: 2000,
  });

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

  return (
    <div
      className="h-screen w-screen overflow-hidden"
      style={{ background: dark ? "#0b1220" : "#f0eee9" }}
    >
      {screen === "setup" ? (
        <Setup
          dark={dark}
          report={setup.data}
          rerunning={setup.isFetching}
          onRerun={() => setup.refetch()}
          onContinue={() => setScreen("pairing")}
          toolbarExtra={toolbarExtra}
          railExtra={
            <DevicesRail
              devices={deviceList}
              error={devices.error}
              selectedUdid={target?.udid}
              onSelect={setSelectedUdid}
            />
          }
        />
      ) : screen === "pairing" ? (
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
          onSkip={() => setScreen("dashboard")}
          onContinue={() => setScreen("dashboard")}
          onBack={() => {
            pair.reset();
            tunnelEstablish.reset();
            wifiCheck.reset();
            setScreen("setup");
          }}
        />
      ) : (
        <Dashboard
          dark={dark}
          live
          device={target ?? null}
          toolbarExtra={toolbarExtra}
          onNavigate={(id) => {
            // Only "Devices" routes anywhere yet — back to pairing. The rest are
            // inert until their phases land.
            if (id === "devices") setScreen("pairing");
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

function DevicesRail({
  devices,
  error,
  selectedUdid,
  onSelect,
}: {
  devices: DeviceInfo[];
  error: unknown;
  selectedUdid?: string;
  onSelect: (udid: string) => void;
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
        <div className="text-[11.5px] text-slate-500">
          {error ? asCommandError(error).remediation : "Plug in your iPhone over USB."}
        </div>
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
