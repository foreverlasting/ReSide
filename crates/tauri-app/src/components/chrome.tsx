import type { ReactNode } from "react";
import { cn } from "../lib/cn";
import type { DeviceInfo } from "../lib/ipc";
import { Icon, Badge, Separator, StatusDot } from "./ui";
import { ReSideMark } from "./logo";

// GNOME / libadwaita-style window chrome wrapper.
export const GnomeWindow = ({
  title,
  subtitle,
  dark = false,
  children,
  toolbar,
  className = "",
  innerClassName = "",
}: {
  title?: ReactNode;
  subtitle?: ReactNode;
  dark?: boolean;
  children?: ReactNode;
  toolbar?: ReactNode;
  className?: string;
  innerClassName?: string;
}) => {
  // `data-theme` lives on a layout-invisible (`display:contents`) wrapper, NOT
  // on the styled panel itself. Tailwind's dark variant and both theme overrides
  // compile to *descendant* selectors (`[data-theme=dark] .dark\:…`), so an
  // element can't theme itself — it must sit BELOW the data-theme node. Keeping
  // them on one div left the window's own background/border/text stuck on their
  // light values in dark mode (the panel stayed white). The wrapper anchors the
  // theme for selector matching while contributing nothing to layout.
  return (
    <div data-theme={dark ? "dark" : "light"} className="contents">
    <div
      className={cn(
        "reside-scope flex h-full w-full flex-col overflow-hidden",
        "rounded-[14px] border",
        "border-slate-300/70 dark:border-slate-700/70",
        "bg-white dark:bg-slate-950",
        "text-slate-900 dark:text-slate-100",
        "shadow-[0_1px_0_0_rgba(255,255,255,0.6)_inset,0_20px_40px_-20px_rgba(15,23,42,0.25)]",
        "font-sans antialiased",
        className
      )}
    >
      {/* Header bar */}
      <div
        className={cn(
          "relative flex h-11 shrink-0 items-center gap-2 px-3",
          "border-b border-slate-200 dark:border-slate-800",
          "bg-gradient-to-b from-slate-50 to-slate-100 dark:from-slate-900 dark:to-slate-900/90"
        )}
      >
        {/* Window controls (libadwaita: minimize / maximize / close) */}
        <div className="flex items-center gap-1.5">
          {(
            [
              ["minimize", "bg-slate-300 hover:bg-slate-400 dark:bg-slate-700 dark:hover:bg-slate-600"],
              ["maximize", "bg-slate-300 hover:bg-slate-400 dark:bg-slate-700 dark:hover:bg-slate-600"],
              ["close", "bg-slate-300 hover:bg-red-500 dark:bg-slate-700 dark:hover:bg-red-500"],
            ] as const
          ).map(([k, cls]) => (
            <button key={k} aria-label={k} className={cn("h-3.5 w-3.5 rounded-full transition-colors", cls)} />
          ))}
        </div>

        {/* Centered title */}
        <div className="pointer-events-none absolute left-1/2 top-0 flex h-full -translate-x-1/2 items-center">
          <div className="text-center leading-tight">
            <div className="text-[12.5px] font-semibold text-slate-800 dark:text-slate-200">{title}</div>
            {subtitle && (
              <div className="text-[10.5px] text-slate-500 dark:text-slate-400 -mt-0.5">{subtitle}</div>
            )}
          </div>
        </div>

        {/* Right side toolbar slot */}
        <div className="ml-auto flex items-center gap-1">{toolbar}</div>
      </div>

      {/* Body */}
      <div className={cn("relative flex-1 min-h-0 overflow-hidden", innerClassName)}>{children}</div>
    </div>
    </div>
  );
};

// Reusable sidebar shell shared by dashboard-like screens.
//
// Gallery mode (the default) renders the mock "Maya's iPhone" + an always-on
// background-agent card. Live mode is opted into by passing `device` (a real
// `DeviceInfo`, or `null` for "no device"); `agentActive` then reflects whether
// the Phase-4 refresh agent is actually running (it isn't yet, so callers pass
// `false`), and `onNavigate` makes the nav items clickable.
export const Sidebar = ({
  active = "apps",
  deviceConnected = true,
  device,
  agentActive = true,
  agentDetail,
  onNavigate,
  noDeviceFallback,
}: {
  active?: string;
  deviceConnected?: boolean;
  device?: DeviceInfo | null;
  agentActive?: boolean;
  agentDetail?: string;
  onNavigate?: (id: string) => void;
  /** Optional replacement for the live-mode "No devices paired" empty state.
   *  Used to surface Wi-Fi reachability + a "Connect over Wi-Fi" button while
   *  the cabled rail is empty. Falls back to the standard hint when omitted. */
  noDeviceFallback?: ReactNode;
}) => {
  // `device === undefined` means gallery mode; otherwise we're live.
  const live = device !== undefined;
  const items: Array<{ id: string; label: string; icon: Parameters<typeof Icon>[0]["name"] }> = [
    { id: "apps", label: "Apps", icon: "package" },
    { id: "devices", label: "Devices", icon: "smartphone" },
    { id: "activity", label: "Activity", icon: "activity" },
    { id: "settings", label: "Settings", icon: "settings" },
  ];
  return (
    <aside className="flex w-[220px] shrink-0 flex-col border-r border-slate-200 bg-slate-50/60 dark:border-slate-800 dark:bg-slate-950">
      {/* Logo */}
      <div className="flex h-12 items-center gap-2 px-4">
        <ReSideMark size={24} className="rounded-[6px]" />
        <div className="text-[14px] font-semibold tracking-tight">ReSide</div>
        <Badge tone="neutral" className="ml-auto text-[10px]">
          v0.4
        </Badge>
      </div>

      <Separator />

      {/* Nav */}
      <nav className="flex flex-col gap-0.5 px-2 py-2">
        {items.map((it) => (
          <button
            key={it.id}
            onClick={() => onNavigate?.(it.id)}
            className={cn(
              "flex h-8 items-center gap-2.5 rounded-md px-2.5 text-[13px]",
              active === it.id
                ? "bg-slate-200/70 text-slate-900 dark:bg-slate-800 dark:text-slate-100"
                : "text-slate-600 hover:bg-slate-200/50 hover:text-slate-900 dark:text-slate-400 dark:hover:bg-slate-800/60 dark:hover:text-slate-100"
            )}
          >
            <Icon name={it.icon} size={14} />
            <span className="font-medium">{it.label}</span>
          </button>
        ))}
      </nav>

      <Separator className="my-2" />

      {/* Devices */}
      <div className="px-3">
        <div className="mb-2 flex items-center justify-between">
          <div className="text-[10.5px] font-semibold uppercase tracking-wider text-slate-500 dark:text-slate-400">
            Devices
          </div>
          <button className="text-slate-400 hover:text-slate-600 dark:hover:text-slate-200">
            <Icon name="plus" size={13} />
          </button>
        </div>
        {live ? (
          device ? (
            <div className="rounded-md border border-slate-200 bg-white p-2.5 dark:border-slate-800 dark:bg-slate-900">
              <div className="flex items-center gap-2">
                <Icon name="smartphone" size={14} className="text-slate-500" />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[12.5px] font-medium">
                    {device.name ?? `${device.udid.slice(0, 8)}…`}
                  </div>
                  <div className="truncate text-[10.5px] text-slate-500">
                    {device.productType ?? "iOS device"}
                    {device.iosVersion ? ` · iOS ${device.iosVersion}` : ""}
                  </div>
                </div>
              </div>
              <div className="mt-2 flex items-center gap-1.5">
                <StatusDot tone={device.supported ? "success" : "danger"} />
                <span className="text-[10.5px] text-slate-500">
                  {device.wifi ? "Wi-Fi" : device.connection.toUpperCase()}
                </span>
              </div>
            </div>
          ) : noDeviceFallback ? (
            <div className="rounded-md border border-dashed border-slate-300 p-2.5 dark:border-slate-700">
              {noDeviceFallback}
            </div>
          ) : (
            <div className="rounded-md border border-dashed border-slate-300 p-2.5 text-[11.5px] text-slate-500 dark:border-slate-700">
              No devices paired
            </div>
          )
        ) : deviceConnected ? (
          <div className="rounded-md border border-slate-200 bg-white p-2.5 dark:border-slate-800 dark:bg-slate-900">
            <div className="flex items-center gap-2">
              <Icon name="smartphone" size={14} className="text-slate-500" />
              <div className="min-w-0 flex-1">
                <div className="truncate text-[12.5px] font-medium">Maya's iPhone</div>
                <div className="truncate text-[10.5px] text-slate-500">iPhone 14 · iOS 17.4</div>
              </div>
            </div>
            <div className="mt-2 flex items-center gap-1.5">
              <StatusDot tone="success" />
              <span className="text-[10.5px] text-slate-500">Wi-Fi · 192.168.1.42</span>
            </div>
          </div>
        ) : (
          <div className="rounded-md border border-dashed border-slate-300 p-2.5 text-[11.5px] text-slate-500 dark:border-slate-700">
            No devices paired
          </div>
        )}
      </div>

      <div className="mt-auto p-3">
        <div className="rounded-md bg-slate-100 p-2.5 dark:bg-slate-900">
          <div className="mb-1.5 flex items-center gap-1.5">
            <Icon name="refresh" size={11} className="text-slate-500" />
            <span className="text-[10.5px] font-medium text-slate-700 dark:text-slate-300">Background agent</span>
            <StatusDot tone={agentActive ? "success" : "neutral"} className="ml-auto" />
          </div>
          <div className="text-[10.5px] text-slate-500">
            {agentDetail ?? (agentActive ? "Running in the background" : "Off — refreshes only while open")}
          </div>
        </div>
      </div>
    </aside>
  );
};
