// Live sign-and-install flow (task 11b). Opened from the Dashboard, this modal:
//   1. opens a native file picker for an .ipa,
//   2. collects Apple ID credentials the first time (stored in the keyring),
//   3. drives the forked Sideloader signer to sign + install over USB,
//   4. prompts for a 2FA code if Apple demands one, and retries.
// Progress is read from the backend `operation_{id}` event stream.

import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { api, asCommandError, type DeviceInfo, type RememberMode } from "../lib/ipc";
import { useOperation, type OperationStage } from "../lib/operations";
import { Button, Input, Label, Icon, Badge, Progress, cn } from "../components/ui";

const STAGE_TEXT: Record<OperationStage, string> = {
  queued: "Queued…",
  preparing: "Reading app…",
  authenticating: "Signing in to Apple…",
  awaiting_2fa: "Waiting for verification code…",
  signing: "Signing (single-threaded for reliability)…",
  transferring: "Transferring to device…",
  installing: "Installing…",
  verifying: "Verifying…",
  trust_required: "Trust the developer on your device",
  done: "Done",
  failed: "Failed",
};

const basename = (p: string) => p.split(/[\\/]/).pop() ?? p;

// The UI offers three tiers; "ask" maps to the backend's in-memory `session`
// store but is discarded when this modal closes (see the cleanup effect).
type RememberChoice = "keyring" | "session" | "ask";

export function ImportModal({
  device,
  onClose,
  onInstalled,
}: {
  device: DeviceInfo | null;
  onClose: () => void;
  onInstalled: () => void;
}) {
  const [opId] = useState(() => `install-${Date.now()}`);
  const [path, setPath] = useState<string | null>(null);
  const [appleId, setAppleId] = useState("");
  const [password, setPassword] = useState("");
  const [twoFa, setTwoFa] = useState("");
  const [needs2fa, setNeeds2fa] = useState(false);
  const [remember, setRemember] = useState<RememberChoice>("keyring");

  const signedIn = useQuery({ queryKey: ["signed-in"], queryFn: api.isSignedIn });
  const credStatus = useQuery({ queryKey: ["cred-status"], queryFn: api.credentialStatus });
  const keyringAvailable = credStatus.data?.keyringAvailable ?? true;
  const op = useOperation(opId);

  // Without a keyring, "on this device" isn't possible — fall back to session.
  useEffect(() => {
    if (!keyringAvailable && remember === "keyring") setRemember("session");
  }, [keyringAvailable, remember]);

  // For "ask every time", discard the in-memory credentials when this modal goes
  // away (whether it closed on success or was cancelled), so they live only for
  // this single install. A ref keeps the latest choice without re-running cleanup.
  const rememberRef = useRef(remember);
  rememberRef.current = remember;
  useEffect(
    () => () => {
      if (rememberRef.current === "ask") void api.signOut();
    },
    []
  );

  // Open the native picker once. Cancelling closes the modal.
  const pickedRef = useRef(false);
  useEffect(() => {
    if (pickedRef.current) return;
    pickedRef.current = true;
    api
      .pickIpa()
      .then((p) => (p ? setPath(p) : onClose()))
      .catch(() => onClose());
  }, [onClose]);

  const install = useMutation({
    mutationFn: async () => {
      if (!signedIn.data && appleId && password) {
        const mode: RememberMode = remember === "keyring" ? "keyring" : "session";
        await api.setAppleCredentials(appleId, password, mode);
        await signedIn.refetch();
      }
      return api.installIpa({
        operationId: opId,
        path: path!,
        udid: device!.udid,
        twoFaCode: needs2fa && twoFa ? twoFa : undefined,
      });
    },
    onSuccess: onInstalled,
    onError: (e) => {
      if (asCommandError(e).category === "AppleAuth2FARequired") setNeeds2fa(true);
    },
  });

  const error = install.error ? asCommandError(install.error) : null;
  // The 2FA error is expected control flow, not a failure to show.
  const showError = error && error.category !== "AppleAuth2FARequired";

  const credsMissing = !signedIn.data && (!appleId || !password);
  const canSubmit =
    !!path && !!device && !install.isPending && !credsMissing && (!needs2fa || twoFa.length > 0);

  return (
    <div className="absolute inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-6 backdrop-blur-sm">
      <div className="w-full max-w-[520px] overflow-hidden rounded-xl border border-slate-200 bg-white shadow-2xl dark:border-slate-700 dark:bg-slate-900">
        {/* Header */}
        <div className="flex items-center gap-2 border-b border-slate-200 px-5 py-3.5 dark:border-slate-800">
          <Icon name="package" size={15} />
          <div className="text-[14px] font-semibold">Install app</div>
          <button
            onClick={onClose}
            aria-label="Close"
            className="ml-auto flex h-7 w-7 items-center justify-center rounded-md text-slate-400 hover:bg-slate-100 hover:text-slate-700 dark:hover:bg-slate-800"
          >
            <Icon name="x" size={14} />
          </button>
        </div>

        <div className="space-y-4 px-5 py-4">
          {/* Selected file */}
          <div>
            <Label className="mb-1.5 block">IPA file</Label>
            <div className="flex items-center gap-2 rounded-md border border-slate-200 bg-slate-50/60 px-3 py-2 text-[13px] dark:border-slate-800 dark:bg-slate-950">
              <Icon name="package" size={13} className="text-slate-500" />
              <span className={cn("truncate font-mono", !path && "text-slate-400")}>
                {path ? basename(path) : "Opening file picker…"}
              </span>
              <button
                onClick={() => api.pickIpa().then((p) => p && setPath(p))}
                className="ml-auto shrink-0 text-[11.5px] font-medium text-slate-600 hover:text-slate-900 dark:text-slate-400 dark:hover:text-slate-100"
              >
                Change
              </button>
            </div>
          </div>

          {/* Target device */}
          <div className="flex items-center gap-2 text-[12.5px] text-slate-600 dark:text-slate-400">
            <Icon name="smartphone" size={13} />
            <span>Install to</span>
            <span className="font-medium text-slate-900 dark:text-slate-100">
              {device ? (device.name ?? `${device.udid.slice(0, 8)}…`) : "No device"}
            </span>
          </div>

          {/* Apple ID — only when not already stored */}
          {signedIn.data === false && !needs2fa && (
            <div className="space-y-3 rounded-lg border border-slate-200 bg-slate-50/60 p-3 dark:border-slate-800 dark:bg-slate-900/40">
              <div className="grid grid-cols-2 gap-3">
                <div>
                  <Label className="mb-1.5 block">Apple ID</Label>
                  <Input
                    value={appleId}
                    onChange={(e) => setAppleId(e.target.value)}
                    placeholder="you@icloud.com"
                    autoComplete="off"
                  />
                </div>
                <div>
                  <Label className="mb-1.5 block">Password</Label>
                  <Input
                    type="password"
                    value={password}
                    onChange={(e) => setPassword(e.target.value)}
                    placeholder="••••••••"
                    autoComplete="off"
                  />
                </div>
              </div>
              {/* How to remember the credentials */}
              <fieldset className="space-y-1.5">
                <legend className="mb-1 text-[12px] font-medium text-slate-700 dark:text-slate-300">
                  Remember my Apple ID?
                </legend>
                <RememberOption
                  checked={remember === "keyring"}
                  disabled={!keyringAvailable}
                  onSelect={() => setRemember("keyring")}
                  title="On this device"
                  desc={
                    keyringAvailable
                      ? "Saved securely in your keyring · enables automatic refresh"
                      : "Unavailable — install a system keyring (gnome-keyring or KWallet)"
                  }
                />
                <RememberOption
                  checked={remember === "session"}
                  onSelect={() => setRemember("session")}
                  title="Just this session"
                  desc="Kept in memory until you quit ReSide; never written to disk"
                />
                <RememberOption
                  checked={remember === "ask"}
                  onSelect={() => setRemember("ask")}
                  title="Don't remember"
                  desc="Ask me every time I install or refresh"
                />
              </fieldset>
              <div className="flex items-center gap-1.5 text-[11px] text-slate-500">
                <Icon name="shieldCheck" size={12} />
                Your password is only ever sent to Apple.
              </div>
            </div>
          )}

          {/* 2FA prompt */}
          {needs2fa && (
            <div className="space-y-2 rounded-lg border border-amber-300 bg-amber-50 p-3 dark:border-amber-800 dark:bg-amber-950/30">
              <div className="flex items-center gap-2">
                <Icon name="shield" size={14} className="text-amber-700 dark:text-amber-300" />
                <div className="text-[13px] font-semibold text-amber-900 dark:text-amber-200">
                  Two-factor code required
                </div>
              </div>
              <p className="text-[12px] text-amber-800 dark:text-amber-300/80">
                Apple sent a code to your trusted devices. Enter it to continue.
              </p>
              <Input
                value={twoFa}
                onChange={(e) => setTwoFa(e.target.value.replace(/\D/g, "").slice(0, 6))}
                placeholder="123456"
                inputMode="numeric"
                className="font-mono tracking-[0.3em]"
              />
            </div>
          )}

          {/* Progress: indeterminate, since the signer reports no measurable %. */}
          {install.isPending && (
            <div className="space-y-1.5">
              <Progress indeterminate />
              <div className="text-[12px] text-slate-500">
                {op ? STAGE_TEXT[op.stage] : "Starting…"}
              </div>
            </div>
          )}

          {/* Error */}
          {showError && (
            <div className="flex items-start gap-2 rounded-md border border-red-200 bg-red-50 px-3 py-2 text-[12px] text-red-700 dark:border-red-900 dark:bg-red-950/30 dark:text-red-300">
              <Icon name="x" size={13} className="mt-0.5 shrink-0" />
              <div>
                <Badge tone="danger" className="mb-1">
                  {error!.category}
                </Badge>
                <div>{error!.remediation}</div>
              </div>
            </div>
          )}
        </div>

        {/* Footer */}
        <div className="flex items-center justify-end gap-2 border-t border-slate-200 bg-slate-50/60 px-5 py-3 dark:border-slate-800 dark:bg-slate-950">
          <Button variant="ghost" size="sm" onClick={onClose} disabled={install.isPending}>
            Cancel
          </Button>
          <Button
            size="sm"
            iconRight="arrowRight"
            disabled={!canSubmit}
            onClick={() => install.mutate()}
          >
            {needs2fa ? "Verify & install" : "Sign & install"}
          </Button>
        </div>
      </div>
    </div>
  );
}

/// One radio row in the "Remember my Apple ID?" group.
function RememberOption({
  checked,
  disabled = false,
  onSelect,
  title,
  desc,
}: {
  checked: boolean;
  disabled?: boolean;
  onSelect: () => void;
  title: string;
  desc: string;
}) {
  return (
    <label
      className={cn(
        "flex items-start gap-2 rounded-md border px-2.5 py-1.5",
        checked
          ? "border-slate-400 bg-white dark:border-slate-600 dark:bg-slate-900"
          : "border-slate-200 dark:border-slate-800",
        disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer"
      )}
    >
      <input
        type="radio"
        name="remember-apple-id"
        className="mt-0.5"
        checked={checked}
        disabled={disabled}
        onChange={onSelect}
      />
      <span className="min-w-0">
        <span className="block text-[12.5px] font-medium text-slate-800 dark:text-slate-200">
          {title}
        </span>
        <span className="block text-[11px] text-slate-500">{desc}</span>
      </span>
    </label>
  );
}
