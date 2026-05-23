// Subscribe to backend operation progress. The Rust side broadcasts
// `operation_{id}` events (see src-tauri/src/lib.rs); long-running flows
// (sign / install / refresh) will drive these. Wired now so the UI half is
// ready the moment a command starts emitting.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

export type OperationStage =
  | "queued"
  | "preparing"
  | "authenticating"
  | "awaiting_2fa"
  | "signing"
  | "transferring"
  | "installing"
  | "verifying"
  | "trust_required"
  | "done"
  | "failed";

export interface OperationEvent {
  id: string;
  stage: OperationStage;
  progress: number;
  message?: string;
  error?: { category: string; remediation: string };
}

/** Track the latest event for a given operation id (or null when inactive). */
export function useOperation(id: string | null): OperationEvent | null {
  const [event, setEvent] = useState<OperationEvent | null>(null);
  useEffect(() => {
    setEvent(null);
    if (!id) return;
    let unlisten: UnlistenFn | undefined;
    let cancelled = false;
    listen<OperationEvent>(`operation_${id}`, (e) => setEvent(e.payload)).then((u) => {
      if (cancelled) u();
      else unlisten = u;
    });
    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, [id]);
  return event;
}
