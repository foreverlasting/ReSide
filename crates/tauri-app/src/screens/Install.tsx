import { GnomeWindow } from "../components/chrome";
import { Icon, Button, Badge, Progress, AppTile, StatusDot, cn } from "../components/ui";

type StageState = "done" | "current" | "error" | "pending";

interface Stage {
  id: string;
  label: string;
  state: StageState;
  detail?: string;
}

export function Install({ dark = false, errorState = false }: { dark?: boolean; errorState?: boolean }) {
  const stages: Stage[] = errorState
    ? [
        { id: "prep", label: "Preparing IPA", state: "done", detail: "Unpacked 38.2 MB · 142 files" },
        { id: "sign", label: "Signing bundle", state: "done", detail: "apple-codesign · cert OK · 1.4s" },
        { id: "transfer", label: "Transferring to device", state: "error", detail: "Device is locked" },
        { id: "install", label: "Installing on device", state: "pending" },
        { id: "verify", label: "Verifying installation", state: "pending" },
      ]
    : [
        { id: "prep", label: "Preparing IPA", state: "done", detail: "Unpacked 38.2 MB · 142 files" },
        { id: "sign", label: "Signing bundle", state: "done", detail: "apple-codesign · cert OK · 1.4s" },
        { id: "transfer", label: "Transferring to device", state: "current", detail: "62% · 24.1 / 38.2 MB · 6.8 MB/s" },
        { id: "install", label: "Installing on device", state: "pending" },
        { id: "verify", label: "Verifying installation", state: "pending" },
      ];

  return (
    <GnomeWindow
      title="ReSide"
      subtitle="Installing Delta"
      dark={dark}
      toolbar={
        <Button variant="ghost" size="icon" aria-label="Minimize">
          <Icon name="x" size={14} />
        </Button>
      }
    >
      <div className="flex h-full flex-col">
        {/* Hero */}
        <div className="flex shrink-0 items-center gap-4 border-b border-slate-200 bg-gradient-to-b from-slate-50 to-white px-7 py-5 dark:border-slate-800 dark:from-slate-900 dark:to-slate-950">
          <AppTile name="Delta" color="violet" size={56} />
          <div className="min-w-0 flex-1">
            <div className="flex items-center gap-2">
              <div className="text-[18px] font-semibold tracking-tight">Delta</div>
              <Badge tone="neutral">v1.6.2</Badge>
              {errorState ? (
                <Badge tone="danger">
                  <StatusDot tone="danger" className="mr-1" />
                  Stuck — device locked
                </Badge>
              ) : (
                <Badge tone="info">
                  <StatusDot tone="info" pulse className="mr-1" />
                  Transferring · 62%
                </Badge>
              )}
            </div>
            <div className="mt-1 flex items-center gap-x-4 gap-y-0.5 flex-wrap text-[11.5px] text-slate-500">
              <div className="flex items-center gap-1.5">
                <Icon name="smartphone" size={12} /> Maya's iPhone
              </div>
              <div className="flex items-center gap-1.5">
                <Icon name="wifi" size={12} /> Wi-Fi · 192.168.1.42
              </div>
              <div className="flex items-center gap-1.5">
                <Icon name="clock" size={12} /> Signing valid for 7 days
              </div>
            </div>
          </div>
          <div className="text-right">
            {errorState ? (
              <div className="text-[24px] font-semibold tabular-nums text-red-600 dark:text-red-400">!</div>
            ) : (
              <>
                <div className="text-[24px] font-semibold tabular-nums">
                  62<span className="text-slate-400">%</span>
                </div>
                <div className="text-[11px] text-slate-500">~38s remaining</div>
              </>
            )}
          </div>
        </div>

        {/* Body: stages */}
        <div className="flex-1 min-h-0 overflow-y-auto px-7 py-6">
          <ol className="relative ml-3">
            {/* Connecting line */}
            <div className="absolute left-3 top-3 bottom-3 w-px bg-slate-200 dark:bg-slate-800" />
            {stages.map((s, i) => (
              <StageRow key={s.id} {...s} last={i === stages.length - 1} />
            ))}
          </ol>

          {/* Error remediation OR Log toggle */}
          {errorState ? (
            <div className="mt-6 overflow-hidden rounded-lg border border-red-300 bg-red-50 dark:border-red-900 dark:bg-red-950/30">
              <div className="flex items-start gap-3 px-5 py-4">
                <div className="mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-red-100 text-red-700 dark:bg-red-900/60 dark:text-red-300">
                  <Icon name="lock" size={15} />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="text-[13.5px] font-semibold text-red-900 dark:text-red-200">
                    Device is locked — we can't transfer while the screen is off
                  </div>
                  <div className="mt-1 text-[12.5px] text-red-800 dark:text-red-300/80">
                    Unlock <span className="font-medium">Maya's iPhone</span>, keep it awake, and click Retry.
                    The signed IPA is ready — we won't need to re-sign.
                  </div>
                  <div className="mt-3 flex items-center gap-2">
                    <Button size="sm" variant="default" iconLeft="rotate">
                      Retry transfer
                    </Button>
                    <Button size="sm" variant="outline" iconLeft="unlock">
                      Send wake-up ping
                    </Button>
                    <Button size="sm" variant="ghost">
                      View log
                    </Button>
                  </div>
                  <div className="mt-3 flex items-center gap-2 text-[11px] font-mono text-red-700/80 dark:text-red-400/70">
                    <span className="text-red-500">error_code</span>
                    <span>lockdown_passcode_required</span>
                  </div>
                </div>
              </div>
            </div>
          ) : (
            <details className="mt-6 rounded-lg border border-slate-200 bg-slate-50/60 dark:border-slate-800 dark:bg-slate-900/40">
              <summary className="flex cursor-pointer items-center gap-2 px-4 py-2.5 text-[12.5px] font-medium">
                <Icon name="terminal" size={13} className="text-slate-500" />
                Live log
                <Badge tone="neutral" className="ml-auto">
                  streaming
                </Badge>
              </summary>
              <div className="border-t border-slate-200 bg-slate-950 px-4 py-3 font-mono text-[11.5px] leading-relaxed text-slate-300 dark:border-slate-800">
                <LogLine ts="14:02:18" tag="prep" msg="ipa unpacked: 142 files, 38.2 MB" />
                <LogLine ts="14:02:19" tag="sign" msg="apple-codesign: bundle id rewritten → com.rileytestut.delta.maya" />
                <LogLine ts="14:02:20" tag="sign" msg="apple-codesign: code signature valid" tone="ok" />
                <LogLine ts="14:02:21" tag="transfer" msg="afc: opened lockdown session on Maya's iPhone" />
                <LogLine ts="14:02:21" tag="transfer" msg="afc: streaming /tmp/reside/delta-resigned.ipa" />
                <LogLine ts="14:02:24" tag="transfer" msg="progress 24.1/38.2 MB · 6.8 MB/s" current />
              </div>
            </details>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-between border-t border-slate-200 bg-slate-50/60 px-7 py-3.5 dark:border-slate-800 dark:bg-slate-950">
          <div className="text-[11.5px] text-slate-500">
            We'll notify you when this finishes — you can close this window.
          </div>
          <div className="flex items-center gap-2">
            <Button variant="ghost" size="sm" iconLeft="pause">
              {errorState ? "Cancel" : "Pause"}
            </Button>
            <Button variant="outline" size="sm">
              {errorState ? "Open log" : "Hide & continue"}
            </Button>
          </div>
        </div>
      </div>
    </GnomeWindow>
  );
}

function StageRow({ label, state, detail, last }: Stage & { last?: boolean }) {
  const dot =
    state === "done" ? (
      <div className="z-10 flex h-6 w-6 items-center justify-center rounded-full bg-emerald-500 ring-4 ring-white dark:ring-slate-950">
        <Icon name="check" size={12} className="text-white" strokeWidth={3} />
      </div>
    ) : state === "current" ? (
      <div className="relative z-10 flex h-6 w-6 items-center justify-center rounded-full bg-sky-500 ring-4 ring-white dark:ring-slate-950">
        <span className="absolute h-6 w-6 animate-ping rounded-full bg-sky-400 opacity-60" />
        <div className="h-2 w-2 rounded-full bg-white" />
      </div>
    ) : state === "error" ? (
      <div className="z-10 flex h-6 w-6 items-center justify-center rounded-full bg-red-500 ring-4 ring-white dark:ring-slate-950">
        <Icon name="x" size={12} className="text-white" strokeWidth={3} />
      </div>
    ) : (
      <div className="z-10 h-6 w-6 rounded-full border-2 border-slate-300 bg-white ring-4 ring-white dark:border-slate-700 dark:bg-slate-950 dark:ring-slate-950" />
    );

  return (
    <li className={cn("relative flex items-start gap-4", !last && "pb-5")}>
      {dot}
      <div className="min-w-0 flex-1 pt-0.5">
        <div
          className={cn(
            "text-[13.5px] font-medium",
            state === "pending"
              ? "text-slate-400 dark:text-slate-500"
              : state === "error"
                ? "text-red-700 dark:text-red-300"
                : "text-slate-900 dark:text-slate-100"
          )}
        >
          {label}
        </div>
        {detail && (
          <div
            className={cn(
              "mt-0.5 font-mono text-[11px]",
              state === "error" ? "text-red-600 dark:text-red-400" : "text-slate-500"
            )}
          >
            {detail}
          </div>
        )}
        {state === "current" && <Progress value={62} className="mt-2 max-w-[420px]" />}
      </div>
    </li>
  );
}

function LogLine({
  ts,
  tag,
  msg,
  tone,
  current,
}: {
  ts: string;
  tag: string;
  msg: string;
  tone?: "ok";
  current?: boolean;
}) {
  const tagColor =
    tone === "ok"
      ? "text-emerald-400"
      : tag === "transfer"
        ? "text-sky-400"
        : tag === "sign"
          ? "text-violet-400"
          : "text-slate-500";
  return (
    <div className={cn("flex gap-3", current && "text-slate-100")}>
      <span className="text-slate-500">{ts}</span>
      <span className={cn("w-16 shrink-0", tagColor)}>{tag}</span>
      <span className="flex-1">{msg}</span>
      {current && <span className="animate-pulse text-sky-400">▍</span>}
    </div>
  );
}
