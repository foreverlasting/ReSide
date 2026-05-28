import type { ReactNode } from "react";
import { GnomeWindow, Sidebar } from "../components/chrome";
import { Icon, Button, Badge, Card, CardContent, Kbd, Separator, AppTile, StatusDot, cn } from "../components/ui";
import type { DeviceInfo, InstalledApp, SetupReport } from "../lib/ipc";

// Everything the live "Get Started" onboarding panel needs. Passed from
// ReSideApp, which derives `systemReady` from the setup check and owns the
// actions (run check, open the detailed Setup/Pairing surfaces on demand,
// enable the background agent). When absent, the dashboard is in gallery mode.
export interface GetStartedHandlers {
  systemReady: boolean;
  report?: SetupReport;
  expanded: boolean;
  rerunning: boolean;
  onRunCheck: () => void;
  onOpenSetup: () => void;
  onPair: () => void;
  onEnableAgent: () => void;
  agentBusy: boolean;
  // Auto-refresh runs unattended, so it needs credentials saved on this device
  // (the keyring tier). False when creds are session-only or not entered, which
  // gates the "enable auto-refresh" affordances.
  canEnableAgent: boolean;
}

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
// replaces the mock search + bell. The app ALWAYS opens here — no gated wizard;
// `getStarted` drives the inline onboarding panel and `onNavigate` reaches the
// on-demand Setup/Pairing surfaces.
export function Dashboard({
  dark = false,
  empty = false,
  live = false,
  device,
  apps = [],
  toolbarExtra,
  onNavigate,
  onImport,
  onRefreshApp,
  onRefreshAll,
  refreshingAll = false,
  agentEnabled = false,
  agentDetail,
  agentBusy = false,
  agentError,
  onToggleAgent,
  getStarted,
  sidebarNoDeviceFallback,
}: {
  dark?: boolean;
  empty?: boolean;
  live?: boolean;
  device?: DeviceInfo | null;
  apps?: InstalledApp[];
  toolbarExtra?: ReactNode;
  onNavigate?: (id: string) => void;
  onImport?: () => void;
  onRefreshApp?: (app: InstalledApp) => void;
  onRefreshAll?: () => void;
  refreshingAll?: boolean;
  agentEnabled?: boolean;
  agentDetail?: string;
  agentBusy?: boolean;
  agentError?: string | null;
  onToggleAgent?: (enabled: boolean) => void;
  getStarted?: GetStartedHandlers;
  /** Replaces the Sidebar's "No devices paired" hint with a richer Wi-Fi
   *  banner + "Connect over Wi-Fi" button. ReSideApp owns the wifi state. */
  sidebarNoDeviceFallback?: ReactNode;
}) {
  const hasApps = live ? apps.length > 0 : !empty;
  const hasDevice = !!device;
  const systemReady = getStarted?.systemReady ?? false;
  // `noApps` keeps the gallery (non-live) branches working unchanged.
  const noApps = live ? !hasApps : empty;
  // Live onboarding is "complete" once a device is paired and the system check
  // passes — that's the friendly empty state. Before that we show Get Started.
  const onboardingComplete = hasDevice && systemReady;
  const showGetStarted = live && !hasApps && !onboardingComplete;

  const deviceLabel = device?.name ?? (device ? `${device.udid.slice(0, 8)}…` : null);

  // Live header copy by stage.
  let liveHeadline = "Installed apps";
  let liveSubhead = "Pair a device to get started.";
  if (hasApps) {
    liveSubhead = `${apps.length} app${apps.length === 1 ? "" : "s"} on ${deviceLabel ?? "your device"}.`;
  } else if (onboardingComplete) {
    liveSubhead = `${deviceLabel} is paired and reachable. Import an IPA to sign and install it.`;
  } else if (systemReady) {
    liveHeadline = "Welcome back";
    liveSubhead = "Your system is ready — pair an iPhone or iPad to start.";
  } else {
    liveHeadline = "Welcome to ReSide";
    liveSubhead = "Three quick things and you'll be installing your first app.";
  }

  const subtitle = live
    ? hasApps && deviceLabel
      ? `Apps · ${deviceLabel}`
      : onboardingComplete
        ? "Ready to install"
        : systemReady
          ? "One more step"
          : "Let's get you set up"
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
          <Sidebar
            active="apps"
            device={device ?? null}
            agentActive={agentEnabled}
            agentDetail={agentDetail}
            onNavigate={onNavigate}
            noDeviceFallback={sidebarNoDeviceFallback}
          />
        ) : (
          <Sidebar active="apps" deviceConnected={!empty} />
        )}

        <main className="flex min-w-0 flex-1 flex-col">
          {/* Header */}
          <div className="flex shrink-0 items-end justify-between gap-6 border-b border-slate-200 px-6 pb-4 pt-5 dark:border-slate-800">
            <div className="min-w-0">
              <h1 className="text-[20px] font-semibold tracking-tight">
                {live ? liveHeadline : empty ? "Welcome to ReSide" : "Installed apps"}
              </h1>
              <p className="mt-0.5 text-[13px] text-slate-500 dark:text-slate-400">
                {live
                  ? liveSubhead
                  : empty
                    ? "Drop an .ipa to get started. We'll handle signing and re-signing every 6 days."
                    : "6 apps on Maya's iPhone · next auto-refresh in 1d 19h"}
              </p>
            </div>
            <div className="flex items-center gap-2">
              {/* "Refresh all" runs the same due-check the agent will (task 11c). */}
              {!noApps && !live && (
                <Button variant="outline" size="sm" iconLeft="refresh">
                  Refresh all
                </Button>
              )}
              {hasApps && live && onRefreshAll && (
                <Button
                  variant="outline"
                  size="sm"
                  iconLeft="refresh"
                  disabled={refreshingAll}
                  onClick={onRefreshAll}
                >
                  {refreshingAll ? "Refreshing…" : "Refresh all due"}
                </Button>
              )}
              {/* The autopilot toggle: installs/removes the systemd timer (or
                  autostart fallback) that runs "Refresh all due" while ReSide
                  is closed. `title` carries the host-specific explanation. */}
              {hasApps && live && onToggleAgent && (
                <Button
                  variant={agentEnabled ? "default" : "outline"}
                  size="sm"
                  iconLeft="refresh"
                  // Can't turn it ON without saved (keyring) credentials; once on,
                  // it stays toggleable so the user can turn it back off.
                  disabled={agentBusy || (!agentEnabled && !(getStarted?.canEnableAgent ?? false))}
                  onClick={() => onToggleAgent(!agentEnabled)}
                  title={
                    !agentEnabled && !(getStarted?.canEnableAgent ?? false)
                      ? "Save your Apple ID on this device (choose it at sign-in) to enable automatic refresh."
                      : (agentError ?? agentDetail)
                  }
                >
                  {agentBusy
                    ? "Saving…"
                    : agentEnabled
                      ? "Auto-refresh: On"
                      : "Auto-refresh: Off"}
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

          {showGetStarted && getStarted ? (
            <GetStartedPanel gs={getStarted} hasDevice={hasDevice} onChoose={onImport} />
          ) : noApps ? (
            <EmptyDashboard live={live} onChoose={onImport} />
          ) : live ? (
            <LiveApps apps={apps} onRefreshApp={onRefreshApp} />
          ) : (
            <FilledDashboard />
          )}
        </main>
      </div>
    </GnomeWindow>
  );
}

// ---------- Get Started panel (live onboarding, shown inside the dashboard) ----------
function GetStartedPanel({
  gs,
  hasDevice,
  onChoose,
}: {
  gs: GetStartedHandlers;
  hasDevice: boolean;
  onChoose?: () => void;
}) {
  const systemDone = gs.systemReady;
  const headline = systemDone ? "One more step" : "Get started";
  const copy = systemDone
    ? "Your system passed all checks. Pair a device to start installing apps."
    : "Run through these once. ReSide handles the rest in the background after that.";

  // Step states derived from live progress.
  const systemState: StepState = systemDone ? "done" : "current";
  const pairState: StepState = hasDevice ? "done" : systemDone ? "current" : "pending";
  const installState: StepState = "pending";
  const doneCount = [systemState, pairState, installState].filter((s) => s === "done").length;

  return (
    <div className="min-h-0 flex-1 overflow-y-auto px-6 py-6">
      <Card className="mx-auto max-w-[760px]">
        <CardContent className="p-6">
          <div className="flex items-start gap-3 pb-4">
            <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-slate-100 dark:bg-slate-800">
              <Icon name="zap" size={15} className="text-slate-700 dark:text-slate-300" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="text-[15px] font-semibold tracking-tight">{headline}</div>
              <div className="mt-1 text-[13px] text-slate-500 dark:text-slate-400">{copy}</div>
            </div>
            <div className="text-right">
              <div className="font-mono text-[18px] font-semibold tabular-nums">
                {doneCount}
                <span className="text-slate-400">/3</span>
              </div>
              <div className="text-[10.5px] text-slate-500">complete</div>
            </div>
          </div>

          <Separator />

          <div className="divide-y divide-slate-200 dark:divide-slate-800">
            <GetStartedStep
              idx={0}
              label="Check system dependencies"
              desc="Verify usbmuxd, libimobiledevice, the signing backend, udev rules, the refresh agent, and notifications."
              state={systemState}
              expanded={gs.expanded}
              gs={gs}
            >
              {systemState === "current" ? (
                gs.expanded ? (
                  <Button size="sm" variant="ghost" iconLeft="refresh" disabled={gs.rerunning} onClick={gs.onRunCheck}>
                    {gs.rerunning ? "Re-running…" : "Re-run"}
                  </Button>
                ) : (
                  <Button size="sm" iconRight="arrowRight" onClick={gs.onRunCheck}>
                    Run check
                  </Button>
                )
              ) : null}
            </GetStartedStep>

            <GetStartedStep
              idx={1}
              label="Pair an iPhone or iPad"
              desc="Plug in by USB and tap Trust on the device."
              state={pairState}
            >
              {pairState === "current" ? (
                <Button size="sm" iconRight="arrowRight" onClick={gs.onPair}>
                  Pair device
                </Button>
              ) : pairState === "pending" ? (
                <Button size="sm" variant="outline" disabled>
                  Pair device
                </Button>
              ) : null}
            </GetStartedStep>

            <GetStartedStep
              idx={2}
              label="Install your first IPA"
              desc="Drop an .ipa file and we'll sign + install it."
              state={installState}
            >
              <Button size="sm" variant="outline" disabled={!hasDevice} onClick={hasDevice ? onChoose : undefined}>
                Choose IPA
              </Button>
            </GetStartedStep>
          </div>
        </CardContent>
      </Card>
    </div>
  );
}

type StepState = "done" | "current" | "pending";

function GetStartedStep({
  idx,
  label,
  desc,
  state,
  expanded = false,
  gs,
  children,
}: {
  idx: number;
  label: string;
  desc: string;
  state: StepState;
  expanded?: boolean;
  gs?: GetStartedHandlers;
  children?: ReactNode;
}) {
  const numClasses =
    state === "done"
      ? "border-emerald-500 bg-emerald-500 text-white"
      : state === "current"
        ? "border-slate-900 bg-slate-900 text-slate-50 dark:border-slate-100 dark:bg-slate-100 dark:text-slate-900"
        : "border-slate-300 bg-white text-slate-400 dark:border-slate-700 dark:bg-slate-900 dark:text-slate-500";

  const warnCount = gs?.report?.warn ?? 0;
  const showInline = state === "current" && expanded && gs;

  return (
    <div className="py-4">
      <div className="flex items-center gap-4">
        <div
          className={cn(
            "flex h-7 w-7 shrink-0 items-center justify-center rounded-full border text-[12px] font-semibold",
            numClasses
          )}
        >
          {state === "done" ? <Icon name="check" size={12} strokeWidth={3} /> : idx + 1}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <div
              className={cn(
                "text-[13.5px] font-medium",
                state === "pending" ? "text-slate-500 dark:text-slate-400" : "text-slate-900 dark:text-slate-100"
              )}
            >
              {label}
            </div>
            {showInline && warnCount > 0 && (
              <Badge tone="warning">
                <StatusDot tone="warning" className="mr-1" />
                {warnCount} {warnCount === 1 ? "warning" : "warnings"}
              </Badge>
            )}
          </div>
          <div className="mt-0.5 text-[12px] text-slate-500 dark:text-slate-400">{desc}</div>
        </div>
        <div className="shrink-0">
          {state === "done" ? (
            <span className="text-[11.5px] font-medium text-emerald-600 dark:text-emerald-400">Done</span>
          ) : (
            children
          )}
        </div>
      </div>

      {showInline && <InlineSystemCheck gs={gs} />}
    </div>
  );
}

function InlineSystemCheck({ gs }: { gs: GetStartedHandlers }) {
  const items = gs.report?.items ?? [];
  return (
    <div className="ml-11 mt-4 rounded-lg border border-slate-200 bg-slate-50/60 dark:border-slate-800 dark:bg-slate-900/40">
      <div className="divide-y divide-slate-200 dark:divide-slate-800">
        {items.length === 0 ? (
          <div className="px-3 py-3 text-[12px] text-slate-500">Running check…</div>
        ) : (
          items.map((c) => (
            <InlineCheckRow
              key={c.key}
              label={c.label}
              // Explain inline why auto-refresh can't be enabled when creds aren't
              // saved on this device, instead of an action the user can't take.
              meta={
                c.key === "agent" && !gs.canEnableAgent
                  ? "Save your Apple ID on this device (at sign-in) to enable auto-refresh"
                  : c.detail
              }
              status={c.status}
              // The one warning we can fix in-app is the background agent — and
              // only when credentials are persisted (keyring) for unattended runs.
              action={
                c.status === "warn" && c.key === "agent" && gs.canEnableAgent
                  ? "Enable agent"
                  : undefined
              }
              onAction={gs.onEnableAgent}
              actionBusy={gs.agentBusy}
            />
          ))
        )}
      </div>
      <div className="flex items-center justify-between gap-3 border-t border-slate-200 px-3 py-2.5 dark:border-slate-800">
        <div className="flex items-center gap-2 text-[11.5px] text-slate-500">
          <Icon name="terminal" size={12} />
          <span>Or fix all from terminal:</span>
          <code className="rounded border border-slate-200 bg-white px-1.5 py-0.5 font-mono text-[10.5px] text-slate-700 dark:border-slate-700 dark:bg-slate-950 dark:text-slate-300">
            reside doctor --fix
          </code>
        </div>
        <Button size="sm" variant="outline" onClick={gs.onOpenSetup}>
          Open detailed view
        </Button>
      </div>
    </div>
  );
}

function InlineCheckRow({
  label,
  meta,
  status,
  action,
  onAction,
  actionBusy = false,
}: {
  label: string;
  meta: string;
  status: "ok" | "warn";
  action?: string;
  onAction?: () => void;
  actionBusy?: boolean;
}) {
  const isOk = status === "ok";
  return (
    <div className="flex items-center gap-3 px-3 py-2.5">
      <div
        className={cn(
          "flex h-6 w-6 shrink-0 items-center justify-center rounded-full",
          isOk
            ? "bg-emerald-50 text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400"
            : "bg-amber-50 text-amber-600 dark:bg-amber-950/40 dark:text-amber-400"
        )}
      >
        <Icon name={isOk ? "check" : "alert"} size={12} strokeWidth={2.5} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[12.5px] font-medium">{label}</div>
        <div className="mt-0 font-mono text-[10.5px] text-slate-500 dark:text-slate-400">{meta}</div>
      </div>
      {action ? (
        <Button
          size="sm"
          variant="outline"
          className="h-7 px-2.5 text-[11.5px]"
          disabled={actionBusy}
          onClick={onAction}
        >
          {actionBusy ? "…" : action}
        </Button>
      ) : (
        <span className="text-[10.5px] font-medium text-emerald-600 dark:text-emerald-400">{isOk ? "OK" : "—"}</span>
      )}
    </div>
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

function LiveApps({
  apps,
  onRefreshApp,
}: {
  apps: InstalledApp[];
  onRefreshApp?: (app: InstalledApp) => void;
}) {
  return (
    <div className="flex-1 min-h-0 overflow-y-auto px-6 py-5">
      <div className="grid grid-cols-2 gap-3">
        {apps.map((a) => (
          <AppCard
            key={a.installationId}
            app={toSampleApp(a)}
            live
            onRefresh={onRefreshApp ? () => onRefreshApp(a) : undefined}
          />
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
              : "Drop an .ipa file anywhere on this window or click the button below. ReSide will sign it with your Apple ID and install it onto your iPhone."}
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

function AppCard({
  app,
  live = false,
  onRefresh,
}: {
  app: SampleApp;
  live?: boolean;
  onRefresh?: () => void;
}) {
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
          {/* Mock cards keep an inert button; live cards wire it to refresh (11c). */}
          {!live ? (
            <button className="text-[11.5px] font-medium text-slate-700 hover:text-slate-900 dark:text-slate-300 dark:hover:text-slate-100">
              Refresh now
            </button>
          ) : (
            onRefresh && (
              <button
                onClick={onRefresh}
                className="text-[11.5px] font-medium text-slate-700 hover:text-slate-900 dark:text-slate-300 dark:hover:text-slate-100"
              >
                Refresh now
              </button>
            )
          )}
        </div>
      </CardContent>
    </Card>
  );
}
