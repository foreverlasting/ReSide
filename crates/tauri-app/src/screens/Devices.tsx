// Devices pane: the persistent home for device management (ROADMAP §7e/§7f).
// Before this, "Devices" pushed a full-screen Pairing overlay that replaced the
// window chrome + sidebar — the one surface that broke the persistent shell
// (§7h). Now it's a *pane*, rendered inside the Dashboard shell's <main> just
// like System / Activity / Settings: no chrome or sidebar of its own.
//
// Single-device-first: one device fills the pane; a compact switcher only
// appears when more than one is connected. The transient trust handshake is a
// focused modal (screens/PairModal.tsx); everything *after* pairing is shown
// here as a "connection ladder" — Paired → Developer Mode → Secure tunnel →
// Wi-Fi refresh — so the state that used to be buried in a wizard is now
// re-visitable, and a blocked rung points at the one thing to fix next.

import type { ReactNode } from "react";
import { Button, Icon, Badge, StatusDot, cn, type IconName } from "../components/ui";
import {
  asCommandError,
  type CommandError,
  type DeviceInfo,
  type TunnelStatus,
  type WifiAvailability,
} from "../lib/ipc";
import type { DevModeState, TunnelPhase, WifiPhase } from "./Pairing";

export function Devices({
  devices,
  selectedUdid,
  onSelect,
  paired,
  developerMode,
  tunnelPhase,
  tunnelStatus,
  tunnelError,
  wifiPhase,
  wifiAvailability,
  wifiError,
  onPair,
  onRecheckDevMode,
  onEstablishTunnel,
  onCheckWifi,
  // Cold-start Wi-Fi reachability (zero connected devices).
  wifiReachable = false,
  wifiChecking = false,
  resolving = false,
  resolveError,
  onConnectWifi,
  onRescanWifi,
}: {
  devices: DeviceInfo[];
  selectedUdid: string | null;
  onSelect: (udid: string) => void;
  /** Whether the selected device has a saved pairing record. */
  paired: boolean;
  developerMode: DevModeState;
  tunnelPhase: TunnelPhase;
  tunnelStatus?: TunnelStatus;
  tunnelError?: CommandError | null;
  wifiPhase: WifiPhase;
  wifiAvailability?: WifiAvailability;
  wifiError?: CommandError | null;
  /** Opens the trust modal. Undefined when no USB device is pairable. */
  onPair?: () => void;
  onRecheckDevMode?: () => void;
  onEstablishTunnel?: () => void;
  onCheckWifi?: () => void;
  wifiReachable?: boolean;
  wifiChecking?: boolean;
  resolving?: boolean;
  resolveError?: CommandError | null;
  onConnectWifi?: () => void;
  onRescanWifi?: () => void;
}) {
  const selected = devices.find((d) => d.udid === selectedUdid) ?? devices[0] ?? null;

  return (
    <>
      <div className="flex shrink-0 items-end justify-between gap-6 border-b border-slate-200 px-8 pb-4 pt-5 dark:border-slate-800">
        <div className="min-w-0">
          <h1 className="text-[20px] font-semibold tracking-tight">Devices</h1>
          <p className="mt-0.5 text-[13px] text-slate-500 dark:text-slate-400">
            Pair iPhones over USB once, then sign &amp; refresh them over Wi-Fi.
          </p>
        </div>
        {devices.length > 0 && (
          <Button
            size="sm"
            iconLeft="plus"
            disabled={!onPair}
            onClick={onPair}
            title={onPair ? undefined : "Connect a device over USB to pair it."}
          >
            Pair new device
          </Button>
        )}
      </div>

      {devices.length === 0 ? (
        <ColdStart
          wifiReachable={wifiReachable}
          wifiChecking={wifiChecking}
          resolving={resolving}
          resolveError={resolveError ?? null}
          onConnectWifi={onConnectWifi}
          onRescanWifi={onRescanWifi}
        />
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-8 py-5">
          {/* Switcher — only when there's more than one device. */}
          {devices.length > 1 && (
            <div className="mb-5 flex flex-wrap items-center gap-2">
              {devices.map((d) => (
                <DeviceChip
                  key={d.udid}
                  device={d}
                  active={d.udid === selected?.udid}
                  onClick={() => onSelect(d.udid)}
                />
              ))}
            </div>
          )}

          {selected && (
            <>
              <DeviceHero device={selected} paired={paired} developerMode={developerMode} />
              <ConnectionLadder
                paired={paired}
                developerMode={developerMode}
                tunnelPhase={tunnelPhase}
                tunnelStatus={tunnelStatus}
                tunnelError={tunnelError ?? null}
                wifiPhase={wifiPhase}
                wifiAvailability={wifiAvailability}
                wifiError={wifiError ?? null}
                onPair={onPair}
                onRecheckDevMode={onRecheckDevMode}
                onEstablishTunnel={onEstablishTunnel}
                onCheckWifi={onCheckWifi}
              />
            </>
          )}
        </div>
      )}
    </>
  );
}

// Overall readiness of a device, for the hero badge + switcher dot.
type Readiness = "ready" | "action" | "unpaired";

function readinessOf(paired: boolean, dev: DevModeState): Readiness {
  if (!paired) return "unpaired";
  // Past pairing, anything not fully "on" still needs the user's attention.
  return dev === "on" ? "ready" : "action";
}

function DeviceChip({
  device,
  active,
  onClick,
}: {
  device: DeviceInfo;
  active: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "inline-flex items-center gap-2.5 rounded-full border py-1.5 pl-2 pr-3 text-[12.5px] transition-colors",
        active
          ? "border-slate-300 bg-slate-100 text-slate-900 dark:border-slate-600 dark:bg-slate-800 dark:text-slate-100"
          : "border-slate-200 bg-white text-slate-600 hover:bg-slate-50 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300 dark:hover:bg-slate-800"
      )}
    >
      <span className="flex h-5 w-5 items-center justify-center rounded-md bg-slate-100 dark:bg-slate-800">
        <Icon name="smartphone" size={12} className="text-slate-500" />
      </span>
      <span className="font-medium">{device.name ?? `${device.udid.slice(0, 8)}…`}</span>
    </button>
  );
}

function DeviceHero({
  device,
  paired,
  developerMode,
}: {
  device: DeviceInfo;
  paired: boolean;
  developerMode: DevModeState;
}) {
  const readiness = readinessOf(paired, developerMode);
  const badge =
    readiness === "ready"
      ? { tone: "success" as const, icon: "check" as IconName, label: "Ready" }
      : readiness === "action"
        ? { tone: "warning" as const, icon: "alert" as IconName, label: "Action needed" }
        : { tone: "neutral" as const, icon: "smartphone" as IconName, label: "Not paired" };

  const conn = device.wifi ? "Wi-Fi" : device.connection.toUpperCase();
  const sub =
    readiness === "ready"
      ? `Connected over ${conn} — signing & refresh ready`
      : readiness === "action"
        ? `Connected over ${conn} · paired, but not ready to refresh yet`
        : `Connected over ${conn} · pair to start`;

  const fields: Array<{ k: string; v: ReactNode; mono?: boolean }> = [
    { k: "iOS", v: device.iosVersion ?? "—" },
    { k: "Model", v: device.productType ?? "—" },
    { k: "Connection", v: conn },
    { k: "UDID", v: `${device.udid.slice(0, 8)}…${device.udid.slice(-4)}`, mono: true },
  ];

  return (
    <div className="rounded-xl border border-slate-200 bg-white p-5 dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center gap-4">
        <div className="flex h-12 w-12 items-center justify-center rounded-xl bg-slate-100 dark:bg-slate-800">
          <Icon name="smartphone" size={22} className="text-slate-500" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2.5">
            <div className="truncate text-[17px] font-semibold tracking-tight">
              {device.name ?? `${device.udid.slice(0, 8)}…`}
            </div>
            <Badge tone={badge.tone}>
              <Icon name={badge.icon} size={11} className="mr-1" strokeWidth={2.5} />
              {badge.label}
            </Badge>
          </div>
          <div className="mt-0.5 text-[12px] text-slate-500 dark:text-slate-400">{sub}</div>
        </div>
      </div>
      <div className="mt-4 grid grid-cols-2 gap-x-6 gap-y-3 sm:grid-cols-4">
        {fields.map((f) => (
          <div key={f.k}>
            <div className="text-[10px] font-medium uppercase tracking-wider text-slate-400">{f.k}</div>
            <div className={cn("mt-1 text-[13px] text-slate-800 dark:text-slate-200", f.mono && "font-mono text-[12px]")}>
              {f.v}
            </div>
          </div>
        ))}
      </div>
    </div>
  );
}

// ---------- Connection ladder ----------

type StepKind = "done" | "current" | "warn" | "loading" | "locked";

function ConnectionLadder({
  paired,
  developerMode,
  tunnelPhase,
  tunnelStatus,
  tunnelError,
  wifiPhase,
  wifiAvailability,
  wifiError,
  onPair,
  onRecheckDevMode,
  onEstablishTunnel,
  onCheckWifi,
}: {
  paired: boolean;
  developerMode: DevModeState;
  tunnelPhase: TunnelPhase;
  tunnelStatus?: TunnelStatus;
  tunnelError: CommandError | null;
  wifiPhase: WifiPhase;
  wifiAvailability?: WifiAvailability;
  wifiError: CommandError | null;
  onPair?: () => void;
  onRecheckDevMode?: () => void;
  onEstablishTunnel?: () => void;
  onCheckWifi?: () => void;
}) {
  const devOn = developerMode === "on";
  const tunnelOn = tunnelPhase === "connected";

  // ---- Step 1: Paired ----
  const s1: StepProps = paired
    ? { kind: "done", title: "Paired", desc: "Trust record saved locally — you won't be asked again." }
    : {
        kind: "current",
        title: "Pair this device",
        desc: "Plug in over USB and tap Trust on the device to save a pairing record.",
        action: onPair && (
          <Button size="sm" onClick={onPair}>
            Pair device
          </Button>
        ),
      };

  // ---- Step 2: Developer Mode ----
  let s2: StepProps;
  if (!paired) {
    s2 = { kind: "locked", title: "Developer Mode", desc: "Available once the device is paired." };
  } else if (developerMode === "checking" || developerMode === "idle") {
    s2 = { kind: "loading", title: "Developer Mode", desc: "Checking Developer Mode…" };
  } else if (devOn) {
    s2 = { kind: "done", title: "Developer Mode", desc: "On — required by iOS 17.4+ for installs." };
  } else {
    const off = developerMode === "off";
    s2 = {
      kind: "warn",
      title: off ? "Developer Mode is off" : "Couldn't read Developer Mode",
      desc: off ? (
        <>
          iOS 17.4+ needs it for installs. On the iPhone:{" "}
          <span className="font-medium text-slate-700 dark:text-slate-200">
            Settings → Privacy &amp; Security → Developer Mode
          </span>
          , toggle on, then restart.
        </>
      ) : (
        "Make sure the device is unlocked and still connected, then re-check."
      ),
      action: onRecheckDevMode && (
        <Button size="sm" variant="outline" iconLeft="refresh" onClick={onRecheckDevMode}>
          Re-check
        </Button>
      ),
    };
  }

  // ---- Step 3: Secure tunnel ----
  let s3: StepProps;
  if (!devOn) {
    s3 = { kind: "locked", title: "Secure tunnel", desc: "Available once Developer Mode is on." };
  } else if (tunnelPhase === "connecting") {
    s3 = { kind: "loading", title: "Secure tunnel", desc: "Establishing an RSD tunnel to the device…" };
  } else if (tunnelOn && tunnelStatus?.endpoint) {
    s3 = {
      kind: "done",
      title: "Secure tunnel",
      desc: (
        <span className="font-mono">
          RSD {tunnelStatus.endpoint.serverAddress}:{tunnelStatus.endpoint.rsdPort} · {tunnelStatus.services.length}{" "}
          services
        </span>
      ),
    };
  } else if (tunnelPhase === "error") {
    s3 = {
      kind: "warn",
      title: "Couldn't establish the tunnel",
      desc: tunnelError?.remediation ?? "The RSD tunnel failed. Try again.",
      action: onEstablishTunnel && (
        <Button size="sm" variant="outline" iconLeft="rotate" onClick={onEstablishTunnel}>
          Retry
        </Button>
      ),
    };
  } else {
    s3 = {
      kind: "current",
      title: "Secure tunnel",
      desc: "Connect to the device's developer services over an RSD tunnel.",
      action: onEstablishTunnel && (
        <Button size="sm" onClick={onEstablishTunnel}>
          Establish tunnel
        </Button>
      ),
    };
  }

  // ---- Step 4: Wi-Fi refresh ----
  let s4: StepProps;
  if (!tunnelOn) {
    s4 = { kind: "locked", title: "Wi-Fi refresh", desc: "Available once the tunnel is established." };
  } else if (wifiPhase === "checking") {
    s4 = { kind: "loading", title: "Wi-Fi refresh", desc: "Scanning the network for RemoteXPC endpoints…" };
  } else if (wifiPhase === "done" && wifiAvailability?.available) {
    const n = wifiAvailability.endpoints.length;
    s4 = {
      kind: "done",
      title: "Wi-Fi refresh",
      desc: `Reachable — ${n} RemoteXPC endpoint${n === 1 ? "" : "s"}. The background agent can re-sign here.`,
    };
  } else if (wifiPhase === "done" && wifiAvailability && !wifiAvailability.available) {
    s4 = {
      kind: "warn",
      title: "Not reachable over Wi-Fi yet",
      desc: "Make sure the device is on the same network and unlocked, then re-check. USB still works.",
      action: onCheckWifi && (
        <Button size="sm" variant="outline" iconLeft="refresh" onClick={onCheckWifi}>
          Re-check
        </Button>
      ),
    };
  } else if (wifiPhase === "error") {
    s4 = {
      kind: "warn",
      title: "Couldn't scan the network",
      desc: wifiError?.remediation ?? "The Wi-Fi scan failed. Try again.",
      action: onCheckWifi && (
        <Button size="sm" variant="outline" iconLeft="rotate" onClick={onCheckWifi}>
          Retry
        </Button>
      ),
    };
  } else {
    s4 = {
      kind: "current",
      title: "Wi-Fi refresh",
      desc: "Check whether this device can be reached over Wi-Fi for background refresh.",
      action: onCheckWifi && (
        <Button size="sm" onClick={onCheckWifi}>
          Check Wi-Fi
        </Button>
      ),
    };
  }

  const steps = [s1, s2, s3, s4];
  const doneCount = steps.filter((s) => s.kind === "done").length;

  return (
    <div className="mt-5">
      <div className="mb-3 flex items-baseline justify-between">
        <div className="text-[13.5px] font-semibold">Connection</div>
        <div className="text-[11.5px] text-slate-500 dark:text-slate-400">
          {doneCount === 4 ? "Ready to sign & refresh over Wi-Fi" : `Step ${Math.min(doneCount + 1, 4)} of 4`}
        </div>
      </div>
      <div>
        {steps.map((s, i) => (
          <LadderStep key={s.title + i} idx={i} last={i === steps.length - 1} {...s} />
        ))}
      </div>
    </div>
  );
}

interface StepProps {
  kind: StepKind;
  title: string;
  desc: ReactNode;
  action?: ReactNode;
}

function LadderStep({
  kind,
  title,
  desc,
  action,
  idx,
  last,
}: StepProps & { idx: number; last: boolean }) {
  const locked = kind === "locked";
  const current = kind === "current";
  const warn = kind === "warn";

  return (
    <div className="relative pb-2.5 last:pb-0">
      {/* Rail connecting to the next node */}
      {!last && (
        <span
          className={cn(
            "absolute left-[16px] top-9 bottom-0 w-px",
            "bg-slate-200 dark:bg-slate-800"
          )}
        />
      )}
      <div
        className={cn(
          "relative flex items-start gap-3.5 rounded-lg border px-3.5 py-3",
          warn
            ? "border-amber-200 bg-amber-50 dark:border-amber-900/60 dark:bg-amber-950/30"
            : current
              ? "border-slate-300 bg-white dark:border-slate-700 dark:bg-slate-900"
              : "border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900",
          locked && "opacity-55"
        )}
      >
        <LadderNode kind={kind} n={idx + 1} />
        <div className="min-w-0 flex-1 pt-0.5">
          <div
            className={cn(
              "text-[13.5px] font-semibold",
              warn ? "text-amber-900 dark:text-amber-200" : "text-slate-900 dark:text-slate-100"
            )}
          >
            {title}
          </div>
          <div
            className={cn(
              "mt-1 text-[11.5px] leading-relaxed",
              warn ? "text-amber-800/90 dark:text-amber-300/90" : "text-slate-500 dark:text-slate-400"
            )}
          >
            {desc}
          </div>
        </div>
        {action && <div className="shrink-0 self-center">{action}</div>}
      </div>
    </div>
  );
}

function LadderNode({ kind, n }: { kind: StepKind; n: number }) {
  if (kind === "done") {
    return (
      <span className="z-[1] flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-emerald-500 bg-emerald-50 text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400">
        <Icon name="check" size={15} strokeWidth={3} />
      </span>
    );
  }
  if (kind === "warn") {
    return (
      <span className="z-[1] flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-amber-400 bg-amber-50 text-amber-600 dark:bg-amber-950/40 dark:text-amber-400">
        <Icon name="alert" size={15} strokeWidth={2.5} />
      </span>
    );
  }
  if (kind === "loading") {
    return (
      <span className="z-[1] flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-slate-300 bg-white text-slate-400 dark:border-slate-700 dark:bg-slate-900">
        <Icon name="refresh" size={14} className="animate-spin" />
      </span>
    );
  }
  if (kind === "locked") {
    return (
      <span className="z-[1] flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-slate-300 bg-white text-slate-400 dark:border-slate-700 dark:bg-slate-900">
        <Icon name="lock" size={13} />
      </span>
    );
  }
  // current
  return (
    <span className="z-[1] flex h-8 w-8 shrink-0 items-center justify-center rounded-full border-2 border-slate-900 bg-slate-900 text-[12px] font-semibold text-slate-50 dark:border-slate-100 dark:bg-slate-100 dark:text-slate-900">
      {n}
    </span>
  );
}

// ---------- Zero-device cold start ----------

function ColdStart({
  wifiReachable,
  wifiChecking,
  resolving,
  resolveError,
  onConnectWifi,
  onRescanWifi,
}: {
  wifiReachable: boolean;
  wifiChecking: boolean;
  resolving: boolean;
  resolveError: CommandError | null;
  onConnectWifi?: () => void;
  onRescanWifi?: () => void;
}) {
  return (
    <div className="flex flex-1 items-center justify-center px-10 py-10">
      <div className="flex w-full max-w-[460px] flex-col items-center text-center">
        <div className="relative flex h-20 w-20 items-center justify-center rounded-3xl border border-slate-200 bg-slate-50 dark:border-slate-800 dark:bg-slate-900">
          <Icon name="smartphone" size={34} className="text-slate-400" />
          <span className="absolute -bottom-1.5 -right-1.5 flex h-8 w-8 items-center justify-center rounded-full border-[3px] border-white bg-slate-900 text-slate-50 dark:border-slate-950 dark:bg-slate-100 dark:text-slate-900">
            <Icon name="plus" size={15} strokeWidth={3} />
          </span>
        </div>
        <h2 className="mt-5 text-[17px] font-semibold tracking-tight">No devices yet</h2>
        <p className="mt-2 max-w-[380px] text-[13px] leading-relaxed text-slate-500 dark:text-slate-400">
          Plug your iPhone in over USB and tap{" "}
          <span className="font-medium text-slate-700 dark:text-slate-200">Trust</span> when it asks.
          After that, ReSide can sign &amp; refresh it over Wi-Fi — no cable needed.
        </p>

        {/* Wi-Fi reachability nudge — mirrors the sidebar's WifiEmptyState (§7i):
            a reachable-but-unpaired iPhone still needs a one-time USB pair. */}
        {resolving ? (
          <div className="mt-6 inline-flex items-center gap-2.5 rounded-lg border border-slate-200 bg-white px-4 py-2.5 text-[12px] text-slate-600 dark:border-slate-800 dark:bg-slate-900 dark:text-slate-300">
            <span className="inline-block h-3.5 w-3.5 animate-spin rounded-full border-2 border-slate-300 border-t-slate-600 dark:border-slate-700 dark:border-t-slate-200" />
            Locating your iPhone over Wi-Fi… first connection takes ~40s.
          </div>
        ) : wifiReachable ? (
          <div className="mt-6 inline-flex items-center gap-2.5 rounded-lg border border-dashed border-slate-300 bg-white px-4 py-2.5 text-[12px] text-slate-600 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-300">
            <StatusDot tone="info" />
            An iPhone is on Wi-Fi — plug it in over USB once to pair it.
            {onRescanWifi && (
              <button
                onClick={onRescanWifi}
                className="ml-1 font-medium text-slate-500 hover:text-slate-800 dark:hover:text-slate-100"
              >
                Rescan
              </button>
            )}
          </div>
        ) : (
          <div className="mt-6 text-[12px] text-slate-500">
            {wifiChecking ? "Looking for iPhones on Wi-Fi…" : "Waiting for a device over USB…"}
          </div>
        )}

        {/* Wi-Fi connect is offered only once a device has been paired before; in
            the zero-device state we never have that, so onConnectWifi is normally
            absent. Kept for symmetry with the sidebar path. */}
        {onConnectWifi && wifiReachable && !resolving && (
          <Button size="sm" variant="outline" iconLeft="wifi" className="mt-3" onClick={onConnectWifi}>
            Connect over Wi-Fi
          </Button>
        )}
        {resolveError && <div className="mt-2 text-[11.5px] text-red-500">{asCommandError(resolveError).remediation}</div>}
      </div>
    </div>
  );
}
