// Activity overlay: a chronological log of what ReSide has done — installs and
// refreshes, including the unattended ones the background agent runs while the
// window is closed. The headline value for a product built around background
// auto-refresh is "what happened while I was away".
//
// Data is already real: `get_activity_log` (Tauri) reads the `activity_log`
// table that `installs` and the refresh scheduler write to (severities
// info/warn/error, operations install/refresh). This screen just surfaces it.
//
// It is a *pane*: ReSideApp renders it inside the persistent Dashboard shell's
// `<main>` (ROADMAP §7h), so no window chrome, sidebar, or "Done" button — the
// sidebar nav is how you leave. The shell's `data-theme` is above it, so plain
// `dark:` utilities work; never add a second `data-theme`.

import { useQuery } from "@tanstack/react-query";
import type { ReactNode } from "react";
import { Button, Badge, Icon, cn } from "../components/ui";
import { api, type ActivityRow } from "../lib/ipc";

export function Activity() {
  const log = useQuery({ queryKey: ["activity-log"], queryFn: api.getActivityLog });
  const rows = log.data ?? [];

  return (
    <>
      <div className="flex items-end justify-between gap-6 px-8 pt-7">
        <div>
          <h1 className="text-[22px] font-semibold tracking-tight">Activity</h1>
          <p className="mt-1.5 text-[13.5px] text-slate-500 dark:text-slate-400">
            {rows.length > 0
              ? `${rows.length} recent event${rows.length === 1 ? "" : "s"}.`
              : "Installs and refreshes will appear here as they happen."}
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          iconLeft="refresh"
          onClick={() => log.refetch()}
          disabled={log.isFetching}
        >
          {log.isFetching ? "Loading…" : "Refresh"}
        </Button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto px-8 py-5">
        {log.isLoading ? (
          <Empty>Reading the activity log…</Empty>
        ) : rows.length === 0 ? (
          <Empty>
            Nothing yet. Once you install an app or a refresh runs — by hand or in
            the background — it shows up here.
          </Empty>
        ) : (
          <ol className="space-y-2">
            {rows.map((r, i) => (
              <ActivityItem key={`${r.ts}-${i}`} row={r} />
            ))}
          </ol>
        )}
      </div>
    </>
  );
}

type Severity = "info" | "warn" | "error";

function severityOf(s: string): Severity {
  return s === "error" ? "error" : s === "warn" ? "warn" : "info";
}

function ActivityItem({ row }: { row: ActivityRow }) {
  const sev = severityOf(row.severity);
  // Info rows take their icon from the operation (install vs refresh); warn/error
  // get a shared attention glyph so problems read at a glance down the column.
  const icon =
    sev === "error"
      ? "x"
      : sev === "warn"
        ? "alert"
        : row.operation === "install"
          ? "package"
          : "refresh";

  const ring =
    sev === "error"
      ? "bg-red-50 text-red-600 dark:bg-red-950/40 dark:text-red-400"
      : sev === "warn"
        ? "bg-amber-50 text-amber-600 dark:bg-amber-950/40 dark:text-amber-400"
        : "bg-emerald-50 text-emerald-600 dark:bg-emerald-950/40 dark:text-emerald-400";

  return (
    <li className="flex items-start gap-3.5 rounded-lg border border-slate-200 bg-white p-3.5 dark:border-slate-800 dark:bg-slate-900">
      <div className={cn("mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center rounded-full", ring)}>
        <Icon name={icon} size={15} strokeWidth={2.25} />
      </div>
      <div className="min-w-0 flex-1">
        <div className="text-[13.5px] text-slate-800 dark:text-slate-200">
          {row.message ?? humanizeFallback(row)}
        </div>
        <div className="mt-1 flex flex-wrap items-center gap-2">
          {row.operation && (
            <Badge tone="neutral" className="capitalize">
              {row.operation}
            </Badge>
          )}
          {sev !== "info" && row.error_category && (
            <Badge tone={sev === "error" ? "danger" : "warning"}>{row.error_category}</Badge>
          )}
          <span className="flex items-center gap-1 text-[11px] text-slate-400">
            <Icon name="clock" size={11} />
            {relativeTime(row.ts)}
          </span>
        </div>
      </div>
    </li>
  );
}

// Defensive fallback if a future writer logs an event with no message.
function humanizeFallback(row: ActivityRow): string {
  const op = row.operation ?? "event";
  return `${op[0].toUpperCase()}${op.slice(1)} — ${row.severity}`;
}

// `ts` is unix *seconds* (the SQLite column the backend writes). Render it as a
// compact relative string; fall back to an absolute date past a week.
function relativeTime(ts: number): string {
  const diffMs = Date.now() - ts * 1000;
  const sec = Math.max(0, Math.floor(diffMs / 1000));
  if (sec < 60) return "just now";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m ago`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h ago`;
  const day = Math.floor(hr / 24);
  if (day < 7) return `${day}d ago`;
  return new Date(ts * 1000).toLocaleDateString();
}

function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-lg border border-dashed border-slate-200 px-4 py-10 text-center text-[12.5px] text-slate-500 dark:border-slate-700 dark:text-slate-400">
      {children}
    </div>
  );
}
