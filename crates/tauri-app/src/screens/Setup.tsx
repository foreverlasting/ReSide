import type { ReactNode } from "react";
import { GnomeWindow } from "../components/chrome";
import { Icon, Button, Badge, cn } from "../components/ui";
import { ReSideMark } from "../components/logo";
import type { SetupReport } from "../lib/ipc";

type CheckStatus = "ok" | "warn";

interface SetupCheck {
  key: string;
  label: string;
  desc: string;
  status: CheckStatus;
  meta: string;
  action?: string;
}

// When `report` is supplied (running inside Tauri) the screen renders live
// backend checks; otherwise it falls back to the design mock (browser gallery).
// `toolbarExtra` (titlebar, before Help) and `railExtra` (bottom of the left
// rail) let the live shell inject status/devices into the chrome rather than
// floating overlays on top of content.
export function Setup({
  dark = false,
  report,
  rerunning = false,
  onRerun,
  onContinue,
  toolbarExtra,
  railExtra,
}: {
  dark?: boolean;
  report?: SetupReport;
  rerunning?: boolean;
  onRerun?: () => void;
  onContinue?: () => void;
  toolbarExtra?: ReactNode;
  railExtra?: ReactNode;
}) {
  const mockChecks: SetupCheck[] = [
    {
      key: "usbmuxd",
      label: "usbmuxd service",
      desc: "Multiplexes USB connections to iOS devices",
      status: "ok",
      meta: "v2.0.2 · active",
    },
    {
      key: "libimobiledevice",
      label: "libimobiledevice",
      desc: "Talks to lockdownd and mounts AFC over usbmux",
      status: "ok",
      meta: "v1.3.0",
    },
    {
      key: "apple-codesign",
      label: "apple-codesign backend",
      desc: "Pure-Rust signing · bundled with ReSide, no system install needed",
      status: "ok",
      meta: "bundled",
    },
    {
      key: "udev",
      label: "udev rules for iOS devices",
      desc: "Lets your user account access /dev/bus/usb without sudo",
      status: "warn",
      meta: "missing · /etc/udev/rules.d/39-libimobiledevice.rules",
      action: "Install rules",
    },
    {
      key: "agent",
      label: "Background refresh agent",
      desc: "systemd --user service that runs daily expiration checks",
      status: "warn",
      meta: "not enabled",
      action: "Enable agent",
    },
    {
      key: "notify",
      label: "Desktop notifications",
      desc: "Detected via org.freedesktop.Notifications",
      status: "ok",
      meta: "GNOME Shell",
    },
  ];

  const checks: SetupCheck[] = report
    ? report.items.map((c) => ({ key: c.key, label: c.label, desc: "", status: c.status, meta: c.detail }))
    : mockChecks;

  const okCount = report ? report.ok : checks.filter((c) => c.status === "ok").length;
  const warnCount = report ? report.warn : checks.filter((c) => c.status === "warn").length;

  return (
    <GnomeWindow
      title="ReSide"
      subtitle="First-run setup"
      dark={dark}
      toolbar={
        <>
          {toolbarExtra}
          <Button variant="ghost" size="icon" aria-label="Help">
            <Icon name="helpCircle" size={14} />
          </Button>
        </>
      }
    >
      <div className="flex h-full">
        {/* Left rail with step indicator */}
        <div className="flex w-[260px] shrink-0 flex-col gap-1 border-r border-slate-200 bg-slate-50/60 px-5 py-6 dark:border-slate-800 dark:bg-slate-950">
          <div className="mb-4 flex items-center gap-2">
            <ReSideMark size={28} className="rounded-[7px]" />
            <div className="text-[14px] font-semibold tracking-tight">ReSide</div>
          </div>
          <div className="mb-2 text-[11px] font-medium uppercase tracking-wider text-slate-500">
            System dependencies
          </div>
          <p className="text-[12px] leading-relaxed text-slate-500 dark:text-slate-400">
            What ReSide needs in place to talk to your iPhone. Re-run this anytime
            from here or the dashboard.
          </p>

          <div className="mt-auto space-y-3">
            {railExtra}
            <div className="rounded-md border border-slate-200 bg-white p-3 text-[11.5px] text-slate-500 dark:border-slate-800 dark:bg-slate-900">
              ReSide stores Apple ID credentials only in your local Secret Service keyring. Nothing is sent off
              this machine.
            </div>
          </div>
        </div>

        {/* Main */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-end justify-between gap-6 px-8 pt-7">
            <div>
              <h1 className="text-[22px] font-semibold tracking-tight">Let's check your system</h1>
              <p className="mt-1.5 text-[13.5px] text-slate-500 dark:text-slate-400">
                ReSide needs a few packages and services in place to talk to your iPhone. We'll only ask once.
              </p>
            </div>
            <div className="flex items-center gap-2 text-[12px] text-slate-500">
              <Badge tone="success">{okCount} ready</Badge>
              <Badge tone="warning">{warnCount} need attention</Badge>
            </div>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto px-8 py-5">
            <div className="space-y-2">
              {checks.map(({ key, ...rest }) => (
                <SetupRow key={key} {...rest} />
              ))}
            </div>

            <div className="mt-6 rounded-lg border border-slate-200 bg-slate-50 p-4 dark:border-slate-800 dark:bg-slate-900/60">
              <div className="flex items-start gap-3">
                <Icon name="terminal" size={16} className="mt-0.5 text-slate-500" />
                <div className="flex-1">
                  <div className="text-[13px] font-medium">Or fix everything from the terminal</div>
                  <div className="mt-2 flex items-center gap-2 rounded-md border border-slate-200 bg-white px-3 py-2 font-mono text-[12px] text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-300">
                    <span className="text-slate-400 select-none">$</span>
                    <span className="flex-1">
                      sudo pacman -S usbmuxd libimobiledevice &amp;&amp; reside doctor --fix
                    </span>
                    <button
                      className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200"
                      aria-label="Copy"
                    >
                      <Icon name="copy" size={13} />
                    </button>
                  </div>
                </div>
              </div>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t border-slate-200 bg-slate-50/60 px-8 py-3.5 dark:border-slate-800 dark:bg-slate-950">
            <Button
              variant="ghost"
              size="sm"
              iconLeft="refresh"
              onClick={onRerun}
              disabled={rerunning}
            >
              {rerunning ? "Re-running…" : "Re-run check"}
            </Button>
            <Button size="sm" onClick={onContinue}>
              Done
            </Button>
          </div>
        </div>
      </div>
    </GnomeWindow>
  );
}

function SetupRow({ label, desc, status, meta, action }: Omit<SetupCheck, "key">) {
  const isOk = status === "ok";
  return (
    <div className="group flex items-center gap-4 rounded-lg border border-slate-200 bg-white p-3.5 transition-colors hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700">
      <div
        className={cn(
          "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
          isOk
            ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400"
            : "bg-amber-50 text-amber-600 dark:bg-amber-950/40 dark:text-amber-400"
        )}
      >
        <Icon name={isOk ? "check" : "alert"} size={15} strokeWidth={2.25} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <div className="text-[13.5px] font-medium">{label}</div>
          <span className="font-mono text-[11px] text-slate-500">{meta}</span>
        </div>
        {desc && <div className="mt-0.5 text-[12px] text-slate-500 dark:text-slate-400">{desc}</div>}
      </div>
      {action && (
        <Button size="sm" variant="outline">
          {action}
        </Button>
      )}
    </div>
  );
}
