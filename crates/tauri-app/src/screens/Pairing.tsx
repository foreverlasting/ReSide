// Design-preview artboard (gallery-only). Imports IPC *types* for shape, but
// the screen itself is not wired to the backend — the live app does not render
// it. See `Gallery.tsx` / `App.tsx`.

import { GnomeWindow } from "../components/chrome";
import { Icon, Button, Badge, Card, CardContent, StatusDot, cn } from "../components/ui";
import type { CommandError, DeviceInfo, TunnelStatus, WifiAvailability } from "../lib/ipc";

export type PairPhase = "idle" | "pairing" | "paired" | "error";
// "idle" = not applicable yet (not paired); the rest mirror the dev-mode query.
export type DevModeState = "idle" | "checking" | "on" | "off" | "unknown";
// Mirrors the establish-tunnel mutation lifecycle.
export type TunnelPhase = "idle" | "connecting" | "connected" | "error";
// Mirrors the check-wifi-availability mutation lifecycle.
export type WifiPhase = "idle" | "checking" | "done" | "error";

// A mock device so the browser design gallery (which passes no props) still
// renders a populated card. The live app always passes a real `device`.
const MOCK_DEVICE: DeviceInfo = {
  udid: "00008110000a1b2c3d4e5f60",
  name: "Maya's iPhone",
  iosVersion: "17.4.1",
  productType: "iPhone15,3",
  connection: "usb",
  wifi: false,
  supported: true,
};

const COPY: Record<PairPhase, { kicker: string; title: string; sub: string }> = {
  idle: {
    kicker: "USB connected",
    title: "Pair your iPhone",
    sub: "Tap Pair to send a trust request. You'll confirm it on the device, then enter your passcode.",
  },
  pairing: {
    kicker: "Waiting for trust",
    title: 'Tap "Trust" on your iPhone',
    sub: "Then enter your device passcode. We'll save the pairing record so you never have to do this again.",
  },
  paired: {
    kicker: "Paired",
    title: "Device trusted",
    sub: "Pairing record saved locally. Wi-Fi refresh setup comes next.",
  },
  error: {
    kicker: "Pairing failed",
    title: "Pairing didn't complete",
    sub: "Resolve the issue below and try again.",
  },
};

export function Pairing({
  dark = false,
  device,
  phase = "idle",
  error,
  developerMode = "idle",
  tunnelPhase = "idle",
  tunnelStatus,
  tunnelError,
  wifiPhase = "idle",
  wifiAvailability,
  wifiError,
  onEstablishTunnel,
  onCheckWifi,
  onPair,
  onRecheckDevMode,
  onSkip,
  onContinue,
  onBack,
}: {
  dark?: boolean;
  device?: DeviceInfo;
  phase?: PairPhase;
  error?: CommandError | null;
  developerMode?: DevModeState;
  tunnelPhase?: TunnelPhase;
  tunnelStatus?: TunnelStatus;
  tunnelError?: CommandError | null;
  wifiPhase?: WifiPhase;
  wifiAvailability?: WifiAvailability;
  wifiError?: CommandError | null;
  onEstablishTunnel?: () => void;
  onCheckWifi?: () => void;
  onPair?: () => void;
  onRecheckDevMode?: () => void;
  onSkip?: () => void;
  onContinue?: () => void;
  onBack?: () => void;
}) {
  // No real device and no mock supplied means the live app is waiting for one.
  const shown = device ?? (onPair ? undefined : MOCK_DEVICE);
  const copy = COPY[phase];
  const sub = phase === "error" && error ? error.remediation : copy.sub;

  const badge =
    phase === "paired"
      ? { tone: "success" as const, label: "Trusted", pulse: false }
      : phase === "pairing"
        ? { tone: "info" as const, label: "Waiting for trust", pulse: true }
        : phase === "error"
          ? { tone: "danger" as const, label: "Not paired", pulse: false }
          : { tone: "neutral" as const, label: "Not paired", pulse: false };

  return (
    <GnomeWindow title="ReSide" subtitle="Pair device" dark={dark}>
      <div className="flex h-full">
        {/* Same step rail as setup for continuity */}
        <div className="flex w-[260px] shrink-0 flex-col gap-1 border-r border-slate-200 bg-slate-50/60 px-5 py-6 dark:border-slate-800 dark:bg-slate-950">
          <div className="mb-4 flex items-center gap-2">
            <div className="flex h-7 w-7 items-center justify-center rounded-md bg-slate-900 text-slate-50 dark:bg-slate-100 dark:text-slate-900">
              <Icon name="refresh" size={14} strokeWidth={2.25} />
            </div>
            <div className="text-[14px] font-semibold tracking-tight">ReSide</div>
          </div>
          <div className="mb-4 text-[11px] font-medium uppercase tracking-wider text-slate-500">
            Setup · step 2 of 3
          </div>
          {(
            [
              { n: 1, label: "System check", state: "done" },
              { n: 2, label: "Pair device", state: "current" },
              { n: 3, label: "First IPA", state: "pending" },
            ] as const
          ).map((s) => (
            <div key={s.n} className="flex items-center gap-3 py-1.5">
              <div
                className={cn(
                  "flex h-6 w-6 items-center justify-center rounded-full border text-[11px] font-semibold",
                  s.state === "current"
                    ? "border-slate-900 bg-slate-900 text-slate-50 dark:border-slate-100 dark:bg-slate-100 dark:text-slate-900"
                    : s.state === "done"
                      ? "border-emerald-500 bg-emerald-500 text-white"
                      : "border-slate-300 bg-white text-slate-500 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-400"
                )}
              >
                {s.state === "done" ? <Icon name="check" size={11} strokeWidth={3} /> : s.n}
              </div>
              <div
                className={cn(
                  "text-[13px]",
                  s.state === "current" ? "font-medium text-slate-900 dark:text-slate-100" : "text-slate-500"
                )}
              >
                {s.label}
              </div>
            </div>
          ))}

          <div className="mt-auto space-y-2">
            <div className="rounded-md border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900">
              <div className="mb-1 flex items-center gap-1.5">
                <Icon name="info" size={12} className="text-slate-400" />
                <div className="text-[11.5px] font-medium">Trouble pairing?</div>
              </div>
              <div className="text-[11px] leading-relaxed text-slate-500">
                Unlock the device, then unplug &amp; replug the USB cable. The trust prompt only appears on an
                unlocked screen.
              </div>
            </div>
          </div>
        </div>

        {/* Main */}
        <div className="relative flex min-w-0 flex-1 flex-col items-center justify-center px-10">
          {/* Subtle dotted background */}
          <div
            className="pointer-events-none absolute inset-0 opacity-[0.35] dark:opacity-20"
            style={{
              backgroundImage: "radial-gradient(circle, rgba(15,23,42,0.12) 1px, transparent 1px)",
              backgroundSize: "18px 18px",
            }}
          />

          <div className="relative z-10 w-full max-w-[520px]">
            <div className="mb-6 text-center">
              <div className="mb-1 text-[11px] font-medium uppercase tracking-wider text-slate-500">
                {copy.kicker}
              </div>
              <h1 className="text-[24px] font-semibold tracking-tight">
                {shown ? copy.title : "Plug in your iPhone"}
              </h1>
              <p className="mt-2 text-[13.5px] text-slate-500 dark:text-slate-400">
                {shown ? sub : "Connect a device over USB to begin pairing."}
              </p>
            </div>

            {/* Phone illustration */}
            <div className="flex items-center justify-center gap-10 py-6">
              <div className="flex flex-col items-center gap-2">
                <Icon name="usb" size={36} className="text-slate-400" />
                <div className="text-[11px] font-mono text-slate-500">USB</div>
              </div>

              <div className="flex flex-col items-center">
                <PhoneSilhouette active={phase === "pairing"} />
              </div>
            </div>

            {/* Device card */}
            {shown && (
              <Card className="mt-2">
                <CardContent className="flex items-center gap-4">
                  <div className="flex h-10 w-10 items-center justify-center rounded-full bg-slate-100 dark:bg-slate-800">
                    <Icon name="smartphone" size={16} className="text-slate-500" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <div className="truncate text-[14px] font-semibold">
                        {shown.name ?? `${shown.udid.slice(0, 8)}…`}
                      </div>
                      <Badge tone={badge.tone}>
                        <StatusDot tone={badge.tone} pulse={badge.pulse} className="mr-1" />
                        {badge.label}
                      </Badge>
                    </div>
                    <div className="mt-0.5 grid grid-cols-3 gap-x-4 text-[11.5px] text-slate-500">
                      <div>
                        <span className="text-slate-400">iOS</span> {shown.iosVersion ?? "—"}
                      </div>
                      <div>
                        <span className="text-slate-400">Model</span> {shown.productType ?? "—"}
                      </div>
                      <div className="truncate font-mono">
                        <span className="text-slate-400">UDID</span> {shown.udid.slice(0, 8)}…
                      </div>
                    </div>
                  </div>
                  {phase !== "paired" && (
                    <Button
                      size="sm"
                      iconLeft={phase === "error" ? "rotate" : undefined}
                      onClick={onPair}
                      disabled={phase === "pairing"}
                    >
                      {phase === "pairing" ? "Waiting…" : phase === "error" ? "Retry" : "Pair device"}
                    </Button>
                  )}
                </CardContent>
              </Card>
            )}

            {phase === "paired" && (
              <DevModeGate state={developerMode} onRecheck={onRecheckDevMode} />
            )}

            {phase === "paired" && developerMode === "on" && (
              <TunnelPanel
                phase={tunnelPhase}
                status={tunnelStatus}
                error={tunnelError}
                onEstablish={onEstablishTunnel}
              />
            )}

            {phase === "paired" && developerMode === "on" && (
              <WifiPanel
                phase={wifiPhase}
                availability={wifiAvailability}
                error={wifiError}
                onCheck={onCheckWifi}
              />
            )}
          </div>

          {/* Footer */}
          <div className="absolute inset-x-0 bottom-0 z-10 flex items-center justify-between border-t border-slate-200 bg-slate-50/60 px-8 py-3.5 dark:border-slate-800 dark:bg-slate-950">
            <Button
              variant="ghost"
              size="sm"
              iconLeft="chevronLeft"
              onClick={onBack}
              disabled={phase === "pairing"}
            >
              Back
            </Button>
            <div className="flex items-center gap-2">
              {/* Both land on the Dashboard. "Enable Wi-Fi refresh" is the happy
                  path once paired; "Skip" goes straight there for USB-only use. */}
              <Button variant="outline" size="sm" onClick={onSkip} disabled={phase === "pairing"}>
                Skip — USB only
              </Button>
              <Button
                size="sm"
                iconRight="arrowRight"
                onClick={onContinue}
                disabled={phase !== "paired"}
              >
                Enable Wi-Fi refresh
              </Button>
            </div>
          </div>
        </div>
      </div>
    </GnomeWindow>
  );
}

// Post-pairing Developer Mode gate. iOS 17.4+ needs Developer Mode on before any
// install flow; remediation copy mirrors the `iOSDeveloperModeOff` taxonomy entry.
function DevModeGate({
  state,
  onRecheck,
}: {
  state: DevModeState;
  onRecheck?: () => void;
}) {
  if (state === "on") {
    return (
      <div className="mt-3 flex items-center gap-2 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-[12.5px] text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950/40 dark:text-emerald-300">
        <Icon name="check" size={15} strokeWidth={2.5} />
        Developer Mode is on — you're ready to install apps.
      </div>
    );
  }

  if (state === "checking" || state === "idle") {
    return (
      <div className="mt-3 flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-4 py-3 text-[12.5px] text-slate-500 dark:border-slate-800 dark:bg-slate-900">
        <Icon name="refresh" size={14} className="animate-spin" />
        Checking Developer Mode…
      </div>
    );
  }

  // "off" or "unknown": block and explain.
  const isOff = state === "off";
  return (
    <div className="mt-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/40">
      <div className="flex items-start gap-2.5">
        <Icon name="alert" size={15} className="mt-0.5 shrink-0 text-amber-600 dark:text-amber-400" />
        <div className="min-w-0 flex-1">
          <div className="text-[12.5px] font-medium text-amber-900 dark:text-amber-200">
            {isOff ? "Developer Mode is off" : "Couldn't read Developer Mode"}
          </div>
          <div className="mt-0.5 text-[11.5px] leading-relaxed text-amber-800/90 dark:text-amber-300/90">
            {isOff
              ? "Enable Developer Mode: Settings → Privacy & Security → Developer Mode, then restart your device."
              : "Make sure the device is unlocked and still connected, then re-check."}
          </div>
        </div>
        <Button variant="outline" size="sm" iconLeft="refresh" onClick={onRecheck}>
          Re-check
        </Button>
      </div>
    </div>
  );
}

// Post-dev-mode RSD tunnel panel. The tunnel (CoreDeviceProxy → software TCP/IP
// stack → RSD handshake) is the gateway to install + Wi-Fi refresh, so it's the
// natural step right after Developer Mode is confirmed on.
function TunnelPanel({
  phase,
  status,
  error,
  onEstablish,
}: {
  phase: TunnelPhase;
  status?: TunnelStatus;
  error?: CommandError | null;
  onEstablish?: () => void;
}) {
  if (phase === "connected" && status?.endpoint) {
    return (
      <div className="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 dark:border-emerald-900 dark:bg-emerald-950/40">
        <div className="flex items-center gap-2 text-[12.5px] font-medium text-emerald-900 dark:text-emerald-200">
          <Icon name="check" size={15} strokeWidth={2.5} />
          Secure tunnel established
        </div>
        <div className="mt-1.5 grid grid-cols-2 gap-x-4 gap-y-0.5 text-[11.5px] text-emerald-800/90 dark:text-emerald-300/90">
          <div className="truncate font-mono">
            <span className="text-emerald-700/70 dark:text-emerald-400/70">RSD</span>{" "}
            {status.endpoint.serverAddress}:{status.endpoint.rsdPort}
          </div>
          <div>
            <span className="text-emerald-700/70 dark:text-emerald-400/70">Services</span>{" "}
            {status.services.length}
          </div>
        </div>
      </div>
    );
  }

  const isError = phase === "error";
  return (
    <div className="mt-3 flex items-center gap-3 rounded-lg border border-slate-200 bg-white px-4 py-3 dark:border-slate-800 dark:bg-slate-900">
      <Icon
        name={isError ? "alert" : "refresh"}
        size={15}
        className={cn(
          "shrink-0",
          isError ? "text-amber-600 dark:text-amber-400" : "text-slate-400",
          phase === "connecting" && "animate-spin"
        )}
      />
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium text-slate-800 dark:text-slate-200">
          {isError ? "Couldn't establish the tunnel" : "Secure tunnel"}
        </div>
        <div className="mt-0.5 text-[11.5px] leading-relaxed text-slate-500">
          {isError && error
            ? error.remediation
            : phase === "connecting"
              ? "Establishing an RSD tunnel to the device…"
              : "Connect to the device's developer services over an RSD tunnel."}
        </div>
      </div>
      <Button
        variant="outline"
        size="sm"
        iconLeft={isError ? "rotate" : undefined}
        onClick={onEstablish}
        disabled={phase === "connecting"}
      >
        {phase === "connecting" ? "Connecting…" : isError ? "Retry" : "Establish tunnel"}
      </Button>
    </div>
  );
}

// Wi-Fi reachability check via mDNS. Discovering a RemoteXPC endpoint on the
// network is the pre-tunnel signal that this device can be refreshed over Wi-Fi
// — the project's headline feature. Not yet device-scoped (see mdns_discovery).
function WifiPanel({
  phase,
  availability,
  error,
  onCheck,
}: {
  phase: WifiPhase;
  availability?: WifiAvailability;
  error?: CommandError | null;
  onCheck?: () => void;
}) {
  if (phase === "done" && availability) {
    const n = availability.endpoints.length;
    if (availability.available) {
      return (
        <div className="mt-3 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 dark:border-emerald-900 dark:bg-emerald-950/40">
          <div className="flex items-center gap-2 text-[12.5px] font-medium text-emerald-900 dark:text-emerald-200">
            <Icon name="check" size={15} strokeWidth={2.5} />
            Reachable over Wi-Fi
          </div>
          <div className="mt-0.5 text-[11.5px] leading-relaxed text-emerald-800/90 dark:text-emerald-300/90">
            Found {n} RemoteXPC endpoint{n === 1 ? "" : "s"} on this network. Background Wi-Fi refresh
            can reach a device here.
          </div>
        </div>
      );
    }
    // Reached the device's services but nothing advertised on the network.
    return (
      <div className="mt-3 flex items-center gap-3 rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 dark:border-amber-900 dark:bg-amber-950/40">
        <Icon name="alert" size={15} className="shrink-0 text-amber-600 dark:text-amber-400" />
        <div className="min-w-0 flex-1 text-[11.5px] leading-relaxed text-amber-800/90 dark:text-amber-300/90">
          No iOS device found on Wi-Fi yet. Make sure the device is on the same network and unlocked,
          then re-check. USB still works.
        </div>
        <Button variant="outline" size="sm" iconLeft="refresh" onClick={onCheck}>
          Re-check
        </Button>
      </div>
    );
  }

  const isError = phase === "error";
  return (
    <div className="mt-3 flex items-center gap-3 rounded-lg border border-slate-200 bg-white px-4 py-3 dark:border-slate-800 dark:bg-slate-900">
      <Icon
        name={isError ? "alert" : "wifi"}
        size={15}
        className={cn(
          "shrink-0",
          isError ? "text-amber-600 dark:text-amber-400" : "text-slate-400",
          phase === "checking" && "animate-spin"
        )}
      />
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium text-slate-800 dark:text-slate-200">
          {isError ? "Couldn't scan the network" : "Wi-Fi reachability"}
        </div>
        <div className="mt-0.5 text-[11.5px] leading-relaxed text-slate-500">
          {isError && error
            ? error.remediation
            : phase === "checking"
              ? "Scanning the network for RemoteXPC endpoints…"
              : "Check whether a device can be reached over Wi-Fi for background refresh."}
        </div>
      </div>
      <Button
        variant="outline"
        size="sm"
        iconLeft={isError ? "rotate" : undefined}
        onClick={onCheck}
        disabled={phase === "checking"}
      >
        {phase === "checking" ? "Scanning…" : isError ? "Retry" : "Check Wi-Fi"}
      </Button>
    </div>
  );
}

// Simple iPhone silhouette with a trust dialog drawn inside.
function PhoneSilhouette({ active = false }: { active?: boolean }) {
  return (
    <div className="relative">
      <div className="h-[180px] w-[100px] rounded-[16px] border-[2.5px] border-slate-300 bg-slate-50 p-1.5 shadow-sm dark:border-slate-700 dark:bg-slate-900">
        <div className="relative h-full w-full overflow-hidden rounded-[10px] bg-slate-100 dark:bg-slate-800">
          {/* Notch */}
          <div className="absolute left-1/2 top-1 h-1.5 w-7 -translate-x-1/2 rounded-full bg-slate-300 dark:bg-slate-700" />
          {/* Dialog */}
          <div className="absolute inset-x-2 top-7 rounded-md bg-white p-1.5 shadow-sm dark:bg-slate-900">
            <div className="text-[6px] font-semibold text-slate-700 dark:text-slate-200">
              Trust This Computer?
            </div>
            <div className="mt-0.5 text-[5px] leading-tight text-slate-500">
              Your settings and data will be accessible.
            </div>
            <div className="mt-1.5 flex gap-1">
              <div className="flex-1 rounded-sm bg-slate-100 py-0.5 text-center text-[5.5px] text-slate-600 dark:bg-slate-800 dark:text-slate-400">
                Don't
              </div>
              <div className="flex-1 rounded-sm bg-sky-500 py-0.5 text-center text-[5.5px] font-semibold text-white">
                Trust
              </div>
            </div>
          </div>
        </div>
      </div>
      {/* Pulsing indicator (only while actively waiting for trust) */}
      {active && (
        <div className="absolute -right-1 -top-1 h-3 w-3">
          <span className="absolute inset-0 animate-ping rounded-full bg-sky-400 opacity-60" />
          <span className="absolute inset-0 rounded-full bg-sky-500 ring-2 ring-white dark:ring-slate-950" />
        </div>
      )}
    </div>
  );
}
