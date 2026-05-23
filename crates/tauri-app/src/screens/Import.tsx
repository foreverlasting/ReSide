import { GnomeWindow, Sidebar } from "../components/chrome";
import { Icon, Button, Badge, Card, CardContent, Input, Label, Checkbox, AppTile, StatusDot, cn, type IconName } from "../components/ui";

export function Import({ dark = false, errorState = false }: { dark?: boolean; errorState?: boolean }) {
  return (
    <GnomeWindow
      title="ReSide"
      subtitle="Import IPA"
      dark={dark}
      toolbar={
        <Button variant="ghost" size="icon" aria-label="Close">
          <Icon name="x" size={14} />
        </Button>
      }
    >
      <div className="flex h-full">
        <Sidebar active="apps" deviceConnected />

        <main className="flex min-w-0 flex-1 flex-col">
          {/* Breadcrumb / header */}
          <div className="flex shrink-0 items-center gap-2 border-b border-slate-200 px-6 py-3 text-[12.5px] text-slate-500 dark:border-slate-800">
            <Icon name="package" size={13} />
            <span>Apps</span>
            <Icon name="chevronRight" size={12} className="text-slate-400" />
            <span className="font-medium text-slate-900 dark:text-slate-100">Import IPA</span>
          </div>

          <div className="flex-1 min-h-0 overflow-y-auto px-6 py-5">
            <div className="mx-auto max-w-[760px]">
              {/* Dropped IPA */}
              <Card>
                <CardContent className="flex items-center gap-4 p-4">
                  <AppTile name="Delta" color="violet" size={52} />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <div className="text-[16px] font-semibold">Delta</div>
                      <Badge tone="neutral">v1.6.2</Badge>
                      <Badge tone="success">Compatible</Badge>
                    </div>
                    <div className="mt-0.5 flex items-center gap-x-4 gap-y-0.5 flex-wrap font-mono text-[11.5px] text-slate-500">
                      <div>
                        <span className="text-slate-400">bundle</span> com.rileytestut.delta
                      </div>
                      <div>
                        <span className="text-slate-400">min</span> iOS 14.0
                      </div>
                      <div>
                        <span className="text-slate-400">arch</span> arm64
                      </div>
                      <div>
                        <span className="text-slate-400">size</span> 38.2 MB
                      </div>
                    </div>
                  </div>
                  <Button variant="ghost" size="sm" iconLeft="refresh">
                    Replace
                  </Button>
                </CardContent>
              </Card>

              {/* Target device */}
              <div className="mt-4">
                <Label className="mb-2 block">Install to</Label>
                <div className="grid grid-cols-2 gap-2">
                  <DeviceRadio selected name="Maya's iPhone" sub="iPhone 14 · iOS 17.4 · Wi-Fi" badge="success" />
                  <DeviceRadio name="Maya's iPad" sub="iPad Pro 11 · iOS 17.3" badge="offline" />
                </div>
              </div>

              {/* Signing method */}
              <div className="mt-5">
                <Label className="mb-2 block">Signing method</Label>
                <div className="grid grid-cols-3 gap-2">
                  <SignMethodCard
                    selected
                    icon="key"
                    title="Free Apple ID"
                    desc="7-day signing window · 3 apps max"
                    badge="Most users"
                  />
                  <SignMethodCard icon="shieldCheck" title="Paid developer cert" desc=".p12 + .mobileprovision · ~1 year" />
                  <SignMethodCard icon="folder" title="Saved profile" desc="0 saved · reuse a prior signing" disabled />
                </div>
              </div>

              {/* Apple ID form OR error block */}
              {errorState ? <TwoFactorError /> : <AppleIDForm />}

              {/* Bundle ID */}
              <div className="mt-5 grid grid-cols-[1fr_auto_auto] items-end gap-2">
                <div>
                  <Label className="mb-1.5 block">Bundle identifier</Label>
                  <Input defaultValue="com.rileytestut.delta.maya" className="font-mono" />
                </div>
                <Button variant="outline" size="md">
                  Reuse original
                </Button>
                <Button variant="outline" size="md">
                  Auto-suffix
                </Button>
              </div>
              <div className="mt-1.5 text-[11.5px] text-slate-500">
                Free Apple IDs require a unique bundle ID — we auto-append your username.
              </div>
            </div>
          </div>

          {/* Footer */}
          <div className="flex items-center justify-between border-t border-slate-200 bg-slate-50/60 px-6 py-3.5 dark:border-slate-800 dark:bg-slate-950">
            <div className="flex items-center gap-2 text-[12px] text-slate-500">
              <Icon name="info" size={13} />
              <span>
                We'll schedule a Wi-Fi refresh every{" "}
                <span className="font-medium text-slate-700 dark:text-slate-300">6 days</span> automatically.
              </span>
            </div>
            <div className="flex items-center gap-2">
              <Button variant="ghost" size="sm">
                Cancel
              </Button>
              <Button size="sm" iconRight="arrowRight" disabled={errorState}>
                Sign &amp; install
              </Button>
            </div>
          </div>
        </main>
      </div>
    </GnomeWindow>
  );
}

function DeviceRadio({
  selected,
  name,
  sub,
  badge,
}: {
  selected?: boolean;
  name: string;
  sub: string;
  badge: "success" | "offline";
}) {
  return (
    <button
      className={cn(
        "flex items-center gap-3 rounded-lg border p-3 text-left transition-colors",
        selected
          ? "border-slate-900 bg-slate-50 dark:border-slate-100 dark:bg-slate-900"
          : "border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700"
      )}
    >
      <div
        className={cn(
          "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
          selected ? "border-slate-900 dark:border-slate-100" : "border-slate-300 dark:border-slate-600"
        )}
      >
        {selected && <div className="h-2 w-2 rounded-full bg-slate-900 dark:bg-slate-100" />}
      </div>
      <Icon name="smartphone" size={14} className="text-slate-500" />
      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium">{name}</div>
        <div className="truncate text-[11.5px] text-slate-500">{sub}</div>
      </div>
      {badge === "success" && <StatusDot tone="success" />}
      {badge === "offline" && <Badge tone="neutral">Offline</Badge>}
    </button>
  );
}

function SignMethodCard({
  icon,
  title,
  desc,
  badge,
  selected,
  disabled,
}: {
  icon: IconName;
  title: string;
  desc: string;
  badge?: string;
  selected?: boolean;
  disabled?: boolean;
}) {
  return (
    <button
      disabled={disabled}
      className={cn(
        "relative flex flex-col gap-2 rounded-lg border p-3 text-left transition-colors disabled:opacity-50",
        selected
          ? "border-slate-900 bg-slate-50 dark:border-slate-100 dark:bg-slate-900"
          : "border-slate-200 bg-white hover:border-slate-300 dark:border-slate-800 dark:bg-slate-900 dark:hover:border-slate-700"
      )}
    >
      {badge && (
        <span className="absolute right-2 top-2 rounded-full bg-slate-900 px-1.5 py-0.5 text-[9.5px] font-medium text-slate-50 dark:bg-slate-100 dark:text-slate-900">
          {badge}
        </span>
      )}
      <div className="flex items-center gap-2">
        <div
          className={cn(
            "flex h-4 w-4 items-center justify-center rounded-full border",
            selected ? "border-slate-900 dark:border-slate-100" : "border-slate-300 dark:border-slate-600"
          )}
        >
          {selected && <div className="h-2 w-2 rounded-full bg-slate-900 dark:bg-slate-100" />}
        </div>
        <Icon name={icon} size={14} className="text-slate-500" />
        <div className="text-[13px] font-medium">{title}</div>
      </div>
      <div className="text-[11.5px] leading-snug text-slate-500">{desc}</div>
    </button>
  );
}

function AppleIDForm() {
  return (
    <div className="mt-5 rounded-lg border border-slate-200 bg-slate-50/60 p-4 dark:border-slate-800 dark:bg-slate-900/40">
      <div className="grid grid-cols-2 gap-3">
        <div>
          <Label className="mb-1.5 block">Apple ID</Label>
          <Input defaultValue="maya@example.com" />
        </div>
        <div>
          <Label className="mb-1.5 block">Password</Label>
          <div className="relative">
            <Input type="password" defaultValue="hunter2hunter2" />
            <button
              className="absolute right-2 top-1/2 -translate-y-1/2 text-slate-400 hover:text-slate-700"
              aria-label="Show password"
            >
              <Icon name="eyeOff" size={14} />
            </button>
          </div>
        </div>
      </div>
      <div className="mt-3 flex items-center justify-between">
        <label className="flex items-center gap-2 text-[12px] text-slate-600 dark:text-slate-400">
          <Checkbox checked />
          Remember in system keyring
        </label>
        <div className="flex items-center gap-1.5 text-[11px] text-slate-500">
          <Icon name="shieldCheck" size={12} />
          Stored locally · Linux Secret Service
        </div>
      </div>
    </div>
  );
}

function TwoFactorError() {
  return (
    <div className="mt-5 overflow-hidden rounded-lg border border-amber-300 bg-amber-50 dark:border-amber-800 dark:bg-amber-950/30">
      <div className="flex items-start gap-3 border-b border-amber-200 px-4 py-3 dark:border-amber-900">
        <div className="mt-0.5 flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-amber-100 text-amber-700 dark:bg-amber-900/60 dark:text-amber-300">
          <Icon name="shield" size={14} />
        </div>
        <div className="min-w-0 flex-1">
          <div className="text-[13.5px] font-semibold text-amber-900 dark:text-amber-200">
            Two-factor authentication required
          </div>
          <div className="mt-0.5 text-[12px] text-amber-800 dark:text-amber-300/80">
            Apple sent a 6-digit code to your trusted devices. Enter it below — or paste an app-specific
            password instead.
          </div>
        </div>
        <Badge tone="warning">apple_2fa_required</Badge>
      </div>
      <div className="grid grid-cols-[1fr_auto] gap-3 px-4 py-3">
        <div>
          <Label className="mb-1.5 block">Verification code</Label>
          <div className="flex items-center gap-1.5">
            {["1", "2", "3", "4", "5", "6"].map((_, i) => (
              <input
                key={i}
                className="h-10 w-10 rounded-md border border-amber-300 bg-white text-center font-mono text-[15px] font-semibold text-slate-900 focus:outline-none focus:ring-2 focus:ring-amber-400 dark:border-amber-800 dark:bg-slate-900 dark:text-slate-100"
                defaultValue={i < 4 ? ["3", "9", "2", "1"][i] : ""}
                maxLength={1}
              />
            ))}
          </div>
        </div>
        <div className="flex flex-col items-end justify-end gap-1">
          <Button size="sm">Verify code</Button>
          <button className="text-[11px] text-slate-500 underline-offset-2 hover:underline">
            Use app-specific password
          </button>
        </div>
      </div>
    </div>
  );
}
