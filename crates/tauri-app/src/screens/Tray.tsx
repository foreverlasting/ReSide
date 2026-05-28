// Design-preview artboard (gallery-only, mock data: "Delta · 5d 9h left",
// etc.). There is no live tray popover yet — this is layout for the planned
// surface. See `Gallery.tsx` / `App.tsx`.

import { Icon, Button, AppTile, StatusDot, cn } from "../components/ui";

type TrayState = "healthy" | "refreshSoon" | "refreshing" | "expired";

export function Tray({ dark = false }: { dark?: boolean }) {
  const apps: Array<{ name: string; color: "violet" | "blue" | "green" | "teal"; state: TrayState; sub: string }> = [
    { name: "Delta", color: "violet", state: "healthy", sub: "5d 9h left" },
    { name: "AltStore", color: "blue", state: "refreshSoon", sub: "1d 19h left" },
    { name: "Provenance", color: "green", state: "refreshing", sub: "refreshing now…" },
    { name: "Feather", color: "teal", state: "expired", sub: "expired 4h ago" },
  ];
  const tones: Record<TrayState, { dot: "success" | "warning" | "info" | "danger"; text: string; pulse?: boolean }> = {
    healthy: { dot: "success", text: "text-emerald-600 dark:text-emerald-400" },
    refreshSoon: { dot: "warning", text: "text-amber-700 dark:text-amber-400" },
    refreshing: { dot: "info", text: "text-sky-600 dark:text-sky-400", pulse: true },
    expired: { dot: "danger", text: "text-red-600 dark:text-red-400" },
  };

  return (
    <div
      data-theme={dark ? "dark" : "light"}
      className={cn(
        "reside-scope flex h-full w-full flex-col overflow-hidden",
        "rounded-[14px] border",
        "border-slate-300/70 dark:border-slate-700/70",
        "bg-white dark:bg-slate-950",
        "text-slate-900 dark:text-slate-100",
        "shadow-[0_30px_60px_-20px_rgba(15,23,42,0.35)]",
        "font-sans antialiased"
      )}
    >
      {/* Pointer caret */}
      <div className="relative">
        <div className="absolute -top-2 right-6 h-3 w-3 rotate-45 border-l border-t border-slate-300 bg-white dark:border-slate-700 dark:bg-slate-950" />
      </div>

      {/* Header */}
      <div className="flex items-center gap-2 border-b border-slate-200 px-4 py-3 dark:border-slate-800">
        <div className="flex h-6 w-6 items-center justify-center rounded-md bg-slate-900 text-slate-50 dark:bg-slate-100 dark:text-slate-900">
          <Icon name="refresh" size={12} strokeWidth={2.25} />
        </div>
        <div className="text-[13px] font-semibold tracking-tight">ReSide</div>
        <StatusDot tone="success" className="ml-1" />
        <div className="ml-auto flex items-center gap-1">
          <Button variant="ghost" size="icon" className="h-7 w-7" aria-label="Settings">
            <Icon name="settings" size={13} />
          </Button>
        </div>
      </div>

      {/* Hero status */}
      <div className="border-b border-slate-200 px-4 py-4 dark:border-slate-800">
        <div className="text-[10.5px] font-medium uppercase tracking-wider text-slate-500">Next auto-refresh</div>
        <div className="mt-0.5 flex items-baseline gap-2">
          <div className="text-[24px] font-semibold tabular-nums tracking-tight">1d 19h</div>
          <div className="text-[12px] text-slate-500">AltStore · Wi-Fi</div>
        </div>
        <div className="mt-2 flex items-center gap-1.5">
          <Icon name="smartphone" size={11} className="text-slate-400" />
          <span className="text-[11px] text-slate-500">Maya's iPhone · reachable</span>
        </div>
      </div>

      {/* App list */}
      <div className="flex-1 min-h-0 overflow-y-auto">
        <div className="flex items-center justify-between px-4 pb-1 pt-3">
          <div className="text-[10.5px] font-medium uppercase tracking-wider text-slate-500">Apps · 4</div>
          <button className="text-[10.5px] text-slate-500 hover:text-slate-800 dark:hover:text-slate-200">
            Refresh all
          </button>
        </div>
        <div className="px-2 pb-2">
          {apps.map((a) => {
            const t = tones[a.state];
            return (
              <div
                key={a.name}
                className="group flex items-center gap-2.5 rounded-md px-2 py-2 hover:bg-slate-100 dark:hover:bg-slate-900"
              >
                <AppTile name={a.name} color={a.color} size={28} />
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[12.5px] font-medium">{a.name}</div>
                  <div className={cn("flex items-center gap-1.5 truncate text-[10.5px]", t.text)}>
                    <StatusDot tone={t.dot} pulse={t.pulse} />
                    {a.sub}
                  </div>
                </div>
                <button
                  className="text-slate-400 opacity-0 transition-opacity hover:text-slate-700 group-hover:opacity-100 dark:hover:text-slate-200"
                  aria-label="More"
                >
                  <Icon name="more" size={13} />
                </button>
              </div>
            );
          })}
        </div>
      </div>

      {/* Footer actions */}
      <div className="border-t border-slate-200 px-3 py-2.5 dark:border-slate-800">
        <div className="flex items-center gap-1.5">
          <Button size="sm" className="flex-1" iconLeft="externalLink">
            Open ReSide
          </Button>
          <Button size="sm" variant="outline" className="h-8 w-8 p-0" aria-label="Pause">
            <Icon name="pause" size={12} />
          </Button>
          <Button size="sm" variant="outline" className="h-8 w-8 p-0" aria-label="Quit">
            <Icon name="power" size={12} />
          </Button>
        </div>
      </div>
    </div>
  );
}
