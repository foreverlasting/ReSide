import type { ReactNode } from "react";
import { GnomeWindow, Sidebar } from "../components/chrome";
import { Icon, Button, Badge, Card, CardContent, Kbd, AppTile, StatusDot, cn } from "../components/ui";
import type { DeviceInfo, InstalledApp } from "../lib/ipc";

type AppStatus = "healthy" | "refreshSoon" | "refreshing" | "expired";

interface SampleApp {
  name: string;
  bundle: string;
  version: string;
  color: "violet" | "blue" | "orange" | "green" | "pink" | "teal";
  expiresDays: number;
  status: AppStatus;
}

const SAMPLE_APPS: SampleApp[] = [
  { name: "Delta", bundle: "com.rileytestut.delta.maya", version: "1.6.2", color: "violet", expiresDays: 5.4, status: "healthy" },
  { name: "AltStore", bundle: "com.rileytestut.altstore.maya", version: "1.7.0", color: "blue", expiresDays: 1.8, status: "refreshSoon" },
  { name: "UTM", bundle: "com.utmapp.utm.maya", version: "4.4.5", color: "orange", expiresDays: 4.1, status: "healthy" },
  { name: "Provenance", bundle: "org.provenance-emu.provenance.maya", version: "2.3.0", color: "green", expiresDays: 0.4, status: "refreshing" },
  { name: "Aidoku", bundle: "app.aidoku.aidoku.maya", version: "0.6.4", color: "pink", expiresDays: 3.0, status: "healthy" },
  { name: "Feather", bundle: "kh.crysalis.feather.maya", version: "1.2.0", color: "teal", expiresDays: -0.2, status: "expired" },
];

// `live` opts into the real app (vs. the design Gallery): apps come from the
// backend (`apps`), the sidebar shows the real `device`, `onImport` opens the
// sign+install flow, and `toolbarExtra` (live status pills + theme toggle)
// replaces the mock search + bell. `onNavigate` lets the sidebar route back to
// pairing.
export function Dashboard({
  dark = false,
  empty = false,
  live = false,
  device,
  apps = [],
  toolbarExtra,
  onNavigate,
  onImport,
}: {
  dark?: boolean;
  empty?: boolean;
  live?: boolean;
  device?: DeviceInfo | null;
  apps?: InstalledApp[];
  toolbarExtra?: ReactNode;
  onNavigate?: (id: string) => void;
  onImport?: () => void;
}) {
  // In live mode the apps area reflects the real install list.
  const noApps = live ? apps.length === 0 : empty;
  const deviceLabel = device?.name ?? (device ? `${device.udid.slice(0, 8)}…` : null);

  const subtitle = live
    ? deviceLabel
      ? `Apps · ${deviceLabel}`
      : "No device"
    : empty
      ? "Welcome"
      : "Apps · Maya's iPhone";

  return (
    <GnomeWindow
      title="ReSide"
      subtitle={subtitle}
      dark={dark}
      toolbar={
        live ? (
          toolbarExtra
        ) : (
          <>
            <div className="mr-2 flex items-center gap-2 rounded-md border border-slate-200 bg-white px-2 py-1 text-[12px] text-slate-500 dark:border-slate-700 dark:bg-slate-900">
              <Icon name="search" size={12} />
              <span className="hidden sm:inline">Search apps</span>
              <Kbd className="ml-2">⌘K</Kbd>
            </div>
            <Button variant="ghost" size="icon" aria-label="Notifications">
              <Icon name="bell" size={14} />
            </Button>
          </>
        )
      }
    >
      <div className="flex h-full">
        {live ? (
          <Sidebar active="apps" device={device ?? null} agentActive={false} onNavigate={onNavigate} />
        ) : (
          <Sidebar active="apps" deviceConnected={!empty} />
        )}

        <main className="flex min-w-0 flex-1 flex-col">
          {/* Header */}
          <div className="flex shrink-0 items-end justify-between gap-6 border-b border-slate-200 px-6 pb-4 pt-5 dark:border-slate-800">
            <div>
              <h1 className="text-[20px] font-semibold tracking-tight">
                {live ? "Your apps" : empty ? "Welcome to ReSide" : "Installed apps"}
              </h1>
              <p className="mt-0.5 text-[13px] text-slate-500 dark:text-slate-400">
                {live
                  ? deviceLabel
                    ? noApps
                      ? `${deviceLabel} is paired and reachable. Import an IPA to sign and install it.`
                      : `${apps.length} app${apps.length === 1 ? "" : "s"} on ${deviceLabel}.`
                    : "Pair a device to get started."
                  : empty
                    ? "Drop an .ipa to get started. We'll handle signing and re-signing every 6 days."
                    : "6 apps on Maya's iPhone · next auto-refresh in 1d 19h"}
              </p>
            </div>
            <div className="flex items-center gap-2">
              {/* "Refresh all" arrives with auto-refresh (11c); hidden in live until then. */}
              {!noApps && !live && (
                <Button variant="outline" size="sm" iconLeft="refresh">
                  Refresh all
                </Button>
              )}
              <Button
                size="sm"
                iconLeft="plus"
                disabled={live && !onImport}
                onClick={live ? onImport : undefined}
              >
                Import IPA
              </Button>
            </div>
          </div>

          {noApps ? (
            <EmptyDashboard live={live} onChoose={onImport} />
          ) : live ? (
            <LiveApps apps={apps} />
          ) : (
            <FilledDashboard />
          )}
        </main>
      </div>
    </GnomeWindow>
  );
}

function FilledDashboard() {
  const filters: Array<{ l: string; n: number; active?: boolean }> = [
    { l: "All", n: 6, active: true },
    { l: "Healthy", n: 4 },
    { l: "Refresh soon", n: 1 },
    { l: "Refreshing", n: 1 },
    { l: "Expired", n: 1 },
  ];
  return (
    <>
      {/* Filter bar */}
      <div className="flex shrink-0 items-center gap-2 border-b border-slate-200 px-6 py-2.5 dark:border-slate-800">
        {filters.map((f) => (
          <button
            key={f.l}
            className={cn(
              "inline-flex items-center gap-1.5 rounded-md px-2.5 py-1 text-[12px] transition-colors",
              f.active
                ? "bg-slate-900 text-slate-50 dark:bg-slate-100 dark:text-slate-900"
                : "text-slate-600 hover:bg-slate-100 dark:text-slate-400 dark:hover:bg-slate-800"
            )}
          >
            <span className="font-medium">{f.l}</span>
            <span className={cn("text-[10.5px]", f.active ? "text-slate-300 dark:text-slate-500" : "text-slate-400")}>
              {f.n}
            </span>
          </button>
        ))}
        <div className="ml-auto flex items-center gap-1.5 text-[11.5px] text-slate-500">
          <span>Sort: Expiration</span>
          <Icon name="chevronDown" size={12} />
        </div>
      </div>

      {/* Grid of app cards */}
      <div className="flex-1 min-h-0 overflow-y-auto px-6 py-5">
        <div className="grid grid-cols-2 gap-3">
          {SAMPLE_APPS.map((app) => (
            <AppCard key={app.bundle} app={app} />
          ))}
        </div>
      </div>
    </>
  );
}

// --- Live (backend-backed) apps ---------------------------------------------

const CARD_COLORS = ["violet", "blue", "orange", "green", "pink", "teal"] as const;

// Stable per-bundle tile color so an app keeps the same look across refreshes.
function colorFor(seed: string): SampleApp["color"] {
  let h = 0;
  for (const ch of seed) h = (h * 31 + ch.charCodeAt(0)) | 0;
  return CARD_COLORS[Math.abs(h) % CARD_COLORS.length];
}

function toSampleApp(a: InstalledApp): SampleApp {
  const expiresDays = (a.expirationTs * 1000 - Date.now()) / 86_400_000;
  const status: AppStatus =
    a.refreshStatus === "refreshing"
      ? "refreshing"
      : expiresDays < 0
        ? "expired"
        : expiresDays < 1.5
          ? "refreshSoon"
          : "healthy";
  return {
    name: a.displayName,
    bundle: a.bundleId,
    version: a.version ?? "—",
    color: colorFor(a.bundleId),
    expiresDays,
    status,
  };
}

function LiveApps({ apps }: { apps: InstalledApp[] }) {
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 py-5">
      <div className="grid grid-cols-2 gap-3">
        {apps.map((a) => (
          <AppCard key={a.installationId} app={toSampleApp(a)} live />
        ))}
      </div>
    </div>
  );
}

function EmptyDashboard({ live = false, onChoose }: { live?: boolean; onChoose?: () => void }) {
  return (
    <div className="flex flex-1 items-center justify-center px-10 py-10">
      <div
        className={cn(
          "flex w-full max-w-[640px] flex-col items-center justify-center gap-4",
          "rounded-2xl border-2 border-dashed border-slate-300 bg-slate-50/40 px-8 py-14 text-center",
          "dark:border-slate-700 dark:bg-slate-900/40"
        )}
      >
        <div className="flex h-14 w-14 items-center justify-center rounded-2xl bg-slate-100 dark:bg-slate-800">
          <Icon name="package" size={22} className="text-slate-500" />
        </div>
        <div>
          <div className="text-[16px] font-semibold">No apps installed yet</div>
          <p className="mx-auto mt-1 max-w-[380px] text-[13px] text-slate-500 dark:text-slate-400">
            {live
              ? "Importing and signing IPAs is the next milestone. Your device is paired and Wi-Fi reachability is confirmed — the refresh foundation is in place."
              : "Drop an .ipa file anywhere on this window or click the button below. ReSide will sign it with your Apple ID and install it over USB."}
          </p>
        </div>
        <div className="mt-1 flex items-center gap-2">
          <Button iconLeft="upload" disabled={live && !onChoose} onClick={live ? onChoose : undefined}>
            Choose IPA…
          </Button>
          <Button variant="outline" disabled={live}>
            Browse examples
          </Button>
        </div>

        <div className="mt-6 flex items-center gap-4 text-[11.5px] text-slate-500">
          <div className="flex items-center gap-1.5">
            <Icon name="shieldCheck" size={13} />
            <span>Credentials stay local</span>
          </div>
          <div className="h-3 w-px bg-slate-300 dark:bg-slate-700" />
          <div className="flex items-center gap-1.5">
            <Icon name="wifi" size={13} />
            <span>Auto-refresh over Wi-Fi</span>
          </div>
          <div className="h-3 w-px bg-slate-300 dark:bg-slate-700" />
          <div className="flex items-center gap-1.5">
            <Icon name="zap" size={13} />
            <span>~1 min per install</span>
          </div>
        </div>
      </div>
    </div>
  );
}

function AppCard({ app, live = false }: { app: SampleApp; live?: boolean }) {
  const { name, bundle, version, color, expiresDays, status } = app;
  const statusMap: Record<AppStatus, { tone: "success" | "warning" | "info" | "danger"; label: string; dot: "success" | "warning" | "info" | "danger"; pulse?: boolean }> = {
    healthy: { tone: "success", label: "Healthy", dot: "success" },
    refreshSoon: { tone: "warning", label: "Refresh soon", dot: "warning" },
    refreshing: { tone: "info", label: "Refreshing now", dot: "info", pulse: true },
    expired: { tone: "danger", label: "Expired", dot: "danger" },
  };
  const s = statusMap[status];

  // Bar fill: full 7 days = 100%. Red zone < 24h.
  const pct = Math.max(0, Math.min(100, (expiresDays / 7) * 100));
  const expiresLabel =
    expiresDays < 0
      ? `Expired ${Math.abs(expiresDays).toFixed(1)}d ago`
      : expiresDays < 1
        ? `Expires in ${Math.round(expiresDays * 24)}h`
        : `Expires in ${expiresDays.toFixed(1)}d`;

  return (
    <Card className="group transition-colors hover:border-slate-300 dark:hover:border-slate-700">
      <CardContent className="p-4">
        <div className="flex items-start gap-3">
          <AppTile name={name} color={color} size={44} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <div className="truncate text-[14px] font-semibold">{name}</div>
              <span className="font-mono text-[11px] text-slate-500">v{version}</span>
            </div>
            <div className="mt-0.5 truncate font-mono text-[11px] text-slate-500">{bundle}</div>
          </div>
          <button className="-mr-1 opacity-0 transition-opacity group-hover:opacity-100" aria-label="More">
            <Icon name="more" size={16} className="text-slate-400 hover:text-slate-700 dark:hover:text-slate-200" />
          </button>
        </div>

        <div className="mt-3.5 flex items-center justify-between gap-2">
          <Badge tone={s.tone}>
            <StatusDot tone={s.dot} pulse={s.pulse} className="mr-1" />
            {s.label}
          </Badge>
          <div
            className={cn(
              "text-[11.5px] font-mono",
              status === "expired"
                ? "text-red-600 dark:text-red-400"
                : status === "refreshSoon"
                  ? "text-amber-700 dark:text-amber-400"
                  : "text-slate-500"
            )}
          >
            {expiresLabel}
          </div>
        </div>

        <div className="mt-2.5 flex items-center gap-2">
          <div className="relative h-1.5 flex-1 overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800">
            {/* danger zone (last day) */}
            <div className="absolute inset-y-0 left-0 w-[14.28%] bg-red-100 dark:bg-red-950/50" />
            <div
              className={cn(
                "absolute inset-y-0 left-0 rounded-full transition-all",
                status === "expired"
                  ? "bg-red-500"
                  : status === "refreshSoon"
                    ? "bg-amber-500"
                    : status === "refreshing"
                      ? "bg-sky-500"
                      : "bg-emerald-500"
              )}
              style={{ width: `${pct}%` }}
            />
          </div>
          {/* Manual refresh lands with auto-refresh (11c); inert button hidden in live. */}
          {!live && (
            <button className="text-[11.5px] font-medium text-slate-700 hover:text-slate-900 dark:text-slate-300 dark:hover:text-slate-100">
              Refresh now
            </button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
