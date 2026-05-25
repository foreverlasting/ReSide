// Manual "Refresh now" flow (task 11c). Opened from an app card on the live
// Dashboard, this re-signs + re-installs the already-stored IPA with the
// already-stored Apple ID — no file pick, no credential entry — and resets the
// app's 7-day free-profile clock. It is the same engine the background agent
// will run on a timer; here it's triggered by hand so the user can watch it.
//
// Progress is read from the backend `operation_{id}` event stream. On a trusted
// machine no 2FA is needed; if Apple ever demands one the command fails loudly
// (it never hangs) and the error is shown rather than a code prompt — re-import
// the app to re-authenticate.

import { useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { api, asCommandError, type InstalledApp } from "../lib/ipc";
import { useOperation, type OperationStage } from "../lib/operations";
import { Button, Icon, Badge, Progress, AppTile } from "../components/ui";

const STAGE_TEXT: Record<OperationStage, string> = {
  queued: "Queued…",
  preparing: "Preparing to re-sign…",
  authenticating: "Signing in to Apple…",
  awaiting_2fa: "Waiting for verification code…",
  signing: "Re-signing (single-threaded for reliability)…",
  transferring: "Transferring to device…",
  installing: "Re-installing…",
  verifying: "Verifying…",
  trust_required: "Trust the developer on your device",
  done: "Refreshed",
  failed: "Failed",
};

export function RefreshModal({
  app,
  onClose,
  onRefreshed,
}: {
  app: InstalledApp;
  onClose: () => void;
  onRefreshed: () => void;
}) {
  const [opId] = useState(() => `refresh-${app.installationId}-${Date.now()}`);
  const op = useOperation(opId);

  const refresh = useMutation({
    mutationFn: () =>
      api.refreshApp({ operationId: opId, installationId: app.installationId }),
    onSuccess: onRefreshed,
  });

  const error = refresh.error ? asCommandError(refresh.error) : null;
  const done = refresh.isSuccess;

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-6 backdrop-blur-sm">
      <div className="w-full max-w-[460px] overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-700 dark:bg-slate-900">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
          <Icon name="refresh" size={15} />
          <div className="text-[14px] font-semibold">Refresh app</div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800"
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="space-y-4 px-5 py-4">
          {/* App identity */}
          <div className="flex items-center gap-3">
            <AppTile name={app.displayName} color="violet" size={40} />
            <div className="min-w-0">
              <div className="truncate text-[14px] font-semibold">{app.displayName}</div>
              <div className="truncate font-mono text-[11px] text-slate-500">{app.bundleId}</div>
            </div>
          </div>

          <p className="text-[12.5px] text-slate-600 dark:text-slate-400">
            Re-signs and re-installs this app with your stored Apple ID, resetting its
            7-day clock. Keep your iPhone connected over USB.
          </p>

          {/* Progress */}
          {refresh.isPending && (
            <div className="space-y-1.5">
              <Progress value={op ? op.progress * 100 : 10} />
              <div className="text-[12px] text-slate-500">
                {op ? STAGE_TEXT[op.stage] : "Starting…"}
              </div>
            </div>
          )}

          {/* Success */}
          {done && (
            <div className="flex items-center gap-2 rounded-md border border-emerald-200 bg-emerald-50 px-3 py-2 text-[12.5px] text-emerald-700 dark:border-emerald-900 dark:bg-emerald-950/30 dark:text-emerald-300">
              <Icon name="shieldCheck" size={14} className="shrink-0" />
              <span>Refreshed — good for another 7 days.</span>
            </div>
          )}

          {/* Error */}
          {error && (
            <div className="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-[12px] text-red-700 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300">
              <Icon name="x" size={13} className="mt-0.5 shrink-0" />
              <div>
                <Badge tone="danger" className="mb-1">
                  {error.category}
                </Badge>
                <div>{error.remediation}</div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-slate-200 bg-slate-50/60 px-5 py-3 dark:border-slate-800 dark:bg-slate-950">
          <Button variant="ghost" size="sm" onClick={onClose} disabled={refresh.isPending}>
            {done ? "Close" : "Cancel"}
          </Button>
          {!done && (
            <Button
              size="sm"
              iconLeft="refresh"
              disabled={refresh.isPending}
              onClick={() => refresh.mutate()}
            >
              {error ? "Try again" : "Refresh now"}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
