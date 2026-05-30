// Focused "Trust this computer" handshake — the one transient step in the
// device workflow. Everything *after* pairing (Developer Mode, the RSD tunnel,
// Wi-Fi reachability) is persistent state and lives in the Devices pane's
// connection ladder (see screens/Devices.tsx); this modal only owns the trust
// handshake itself. It renders as a sibling of the Dashboard, sharing the
// ImportModal / RefreshModal pattern (and the §7j dark-scrim fix). The
// gallery-only design artboard still lives in screens/Pairing.tsx.

import type { CommandError, DeviceInfo } from "../lib/ipc";
import { Button, Icon, Badge, StatusDot } from "../components/ui";
import type { PairPhase } from "./Pairing";

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
    sub: "Pairing record saved locally. Back on Devices you can finish setting up Wi-Fi refresh.",
  },
  error: {
    kicker: "Pairing failed",
    title: "Pairing didn't complete",
    sub: "Resolve the issue below and try again.",
  },
};

export function PairModal({
  device,
  phase = "idle",
  error,
  onPair,
  onClose,
}: {
  device: DeviceInfo | null;
  phase?: PairPhase;
  error?: CommandError | null;
  onPair?: () => void;
  onClose: () => void;
}) {
  const copy = COPY[phase];
  const sub = phase === "error" && error ? error.remediation : copy.sub;
  const busy = phase === "pairing";

  const badge =
    phase === "paired"
      ? { tone: "success" as const, label: "Trusted", pulse: false }
      : phase === "pairing"
        ? { tone: "info" as const, label: "Waiting for trust", pulse: true }
        : phase === "error"
          ? { tone: "danger" as const, label: "Not paired", pulse: false }
          : { tone: "neutral" as const, label: "Not paired", pulse: false };

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-slate-900/40 dark:bg-slate-950/80 p-6 backdrop-blur-sm">
      <div className="w-full max-w-[460px] overflow-hidden rounded-xl border border-slate-200 bg-white text-slate-900 shadow-2xl dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
          <Icon name="smartphone" size={15} />
          <div className="text-[14px] font-semibold">Pair device</div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800 dark:hover:text-slate-100"
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="px-6 py-5">
          {/* Headline */}
          <div className="text-center">
            <div className="mb-1 text-[10.5px] font-medium uppercase tracking-wider text-slate-500">
              {copy.kicker}
            </div>
            <h2 className="text-[18px] font-semibold tracking-tight">
              {device ? copy.title : "Plug in your iPhone"}
            </h2>
            <p className="mx-auto mt-1.5 max-w-[360px] text-[12.5px] text-slate-500 dark:text-slate-400">
              {device ? sub : "Connect a device over USB to begin pairing."}
            </p>
          </div>

          {/* Illustration */}
          <div className="flex items-center justify-center gap-8 py-5">
            <div className="flex flex-col items-center gap-1.5">
              <Icon name="usb" size={30} className="text-slate-400" />
              <div className="font-mono text-[10.5px] text-slate-500">USB</div>
            </div>
            <PhoneSilhouette active={busy} />
          </div>

          {/* Device card */}
          {device && (
            <div className="rounded-lg border border-slate-200 bg-slate-50/60 p-3.5 dark:border-slate-800 dark:bg-slate-950">
              <div className="flex items-center gap-3">
                <div className="flex h-9 w-9 items-center justify-center rounded-full bg-slate-100 dark:bg-slate-800">
                  <Icon name="smartphone" size={15} className="text-slate-500" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <div className="truncate text-[13.5px] font-semibold">
                      {device.name ?? `${device.udid.slice(0, 8)}…`}
                    </div>
                    <Badge tone={badge.tone}>
                      <StatusDot tone={badge.tone} pulse={badge.pulse} className="mr-1" />
                      {badge.label}
                    </Badge>
                  </div>
                  <div className="mt-0.5 truncate text-[11px] text-slate-500">
                    {device.productType ?? "iOS device"}
                    {device.iosVersion ? ` · iOS ${device.iosVersion}` : ""}
                  </div>
                </div>
              </div>
            </div>
          )}

          {/* Trouble hint — only while idle/pairing */}
          {device && phase !== "paired" && (
            <div className="mt-3 flex items-start gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2.5 dark:border-slate-800 dark:bg-slate-900">
              <Icon name="info" size={13} className="mt-0.5 shrink-0 text-slate-400" />
              <div className="text-[11px] leading-relaxed text-slate-500">
                No trust prompt? Unlock the device, then unplug &amp; replug the cable — it only appears
                on an unlocked screen.
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-slate-200 bg-slate-50/60 px-5 py-3 dark:border-slate-800 dark:bg-slate-950">
          {phase === "paired" ? (
            <Button size="sm" iconRight="arrowRight" onClick={onClose}>
              Done
            </Button>
          ) : (
            <>
              <Button variant="ghost" size="sm" onClick={onClose} disabled={busy}>
                Cancel
              </Button>
              <Button
                size="sm"
                iconLeft={phase === "error" ? "rotate" : undefined}
                disabled={!device || busy || !onPair}
                onClick={onPair}
              >
                {busy ? "Waiting…" : phase === "error" ? "Retry" : "Pair device"}
              </Button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}

// iPhone silhouette with a trust dialog drawn inside (moved from Pairing.tsx so
// the live modal doesn't depend on the gallery artboard's internals).
function PhoneSilhouette({ active = false }: { active?: boolean }) {
  return (
    <div className="relative">
      <div className="h-[150px] w-[84px] rounded-[14px] border-[2.5px] border-slate-300 bg-slate-50 p-1.5 shadow-sm dark:border-slate-700 dark:bg-slate-900">
        <div className="relative h-full w-full overflow-hidden rounded-[9px] bg-slate-100 dark:bg-slate-800">
          {/* Notch */}
          <div className="absolute left-1/2 top-1 h-1.5 w-6 -translate-x-1/2 rounded-full bg-slate-300 dark:bg-slate-700" />
          {/* Dialog */}
          <div className="absolute inset-x-1.5 top-6 rounded-md bg-white p-1.5 shadow-sm dark:bg-slate-900">
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
      {active && (
        <div className="absolute -right-1 -top-1 h-3 w-3">
          <span className="absolute inset-0 animate-ping rounded-full bg-sky-400 opacity-60" />
          <span className="absolute inset-0 rounded-full bg-sky-500 ring-2 ring-white dark:ring-slate-950" />
        </div>
      )}
    </div>
  );
}
