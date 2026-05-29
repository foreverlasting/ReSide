// Settings overlay: certificate management + Apple ID credentials.
//
// The headline job is unsticking a user who hit Apple's free-account cap of ~2
// development certificates — signing then fails with no way out (see ROADMAP
// item 1). This screen lists the account's certs and revokes one, and lets the
// user change or forget their stored Apple ID. It owns its own data (queries +
// mutations) so the shell only has to mount it. Theming note: this renders
// inside `GnomeWindow`, which sets `data-theme`, so we use plain `dark:`
// utilities here and never a second `data-theme`.

import { useState, type ReactNode } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { GnomeWindow } from "../components/chrome";
import { ReSideMark } from "../components/logo";
import {
  Button,
  Input,
  Label,
  Badge,
  Icon,
  Separator,
  cn,
} from "../components/ui";
import { api, asCommandError, type RememberMode } from "../lib/ipc";

export function Settings({
  dark = false,
  onClose,
  toolbarExtra,
  railExtra,
}: {
  dark?: boolean;
  onClose?: () => void;
  toolbarExtra?: ReactNode;
  railExtra?: ReactNode;
}) {
  const qc = useQueryClient();

  const creds = useQuery({ queryKey: ["cred-status"], queryFn: api.credentialStatus });
  const signedIn = creds.data && creds.data.mode !== "none";

  // A 2FA code, entered only if Apple challenges a cert list/revoke. Held for the
  // rest of this screen's cert calls so a verified user isn't re-prompted on each
  // action. A trusted device usually skips this entirely.
  const [twoFa, setTwoFa] = useState("");

  // Certs require a logged-in Apple account; only fetch once we have one, so a
  // signed-out user sees "sign in first" rather than a confusing auth error.
  const certs = useQuery({
    queryKey: ["certificates"],
    queryFn: () => api.listCertificates(twoFa || undefined),
    enabled: !!signedIn,
    retry: false,
  });

  // 2FA is expected control flow, not a failure — surface the code input when the
  // list query reports it. A revoke that hits 2FA refetches the list on settle,
  // which routes the challenge back through here too, so one source covers both.
  const needs2fa =
    !!certs.error && asCommandError(certs.error).category === "AppleAuth2FARequired";

  // Which serial is mid-revoke, so we disable just that row's button.
  const [revoking, setRevoking] = useState<string | null>(null);
  const revoke = useMutation({
    mutationFn: (serial: string) => {
      setRevoking(serial);
      return api.revokeCertificate(serial, twoFa || undefined);
    },
    onSettled: () => {
      setRevoking(null);
      certs.refetch();
    },
  });

  const forget = useMutation({
    mutationFn: () => api.signOut(),
    onSuccess: () => {
      setTwoFa("");
      qc.invalidateQueries({ queryKey: ["cred-status"] });
      qc.setQueryData(["certificates"], []);
    },
  });

  return (
    <GnomeWindow
      title="ReSide"
      subtitle="Settings"
      dark={dark}
      toolbar={
        <>
          {toolbarExtra}
          <Button variant="ghost" size="icon" aria-label="Help">
            <Icon name="helpCircle" size={14} />
          </Button>
        </>
      }
    >
      <div className="flex h-full">
        {/* Left rail */}
        <div className="flex w-[260px] shrink-0 flex-col gap-1 border-r border-slate-200 bg-slate-50/60 px-5 py-6 dark:border-slate-800 dark:bg-slate-950">
          <div className="mb-4 flex items-center gap-2">
            <ReSideMark size={28} className="rounded-[7px]" />
            <div className="text-[14px] font-semibold tracking-tight">ReSide</div>
          </div>
          <div className="mb-2 text-[11px] font-medium uppercase tracking-wider text-slate-500">
            Settings
          </div>
          <p className="text-[12px] leading-relaxed text-slate-500 dark:text-slate-400">
            Manage the signing certificates on your Apple account and the Apple ID
            ReSide signs with.
          </p>
          <div className="mt-auto space-y-3">
            {railExtra}
            <div className="rounded-md border border-slate-200 bg-white p-3 text-[11.5px] text-slate-500 dark:border-slate-800 dark:bg-slate-900">
              Your Apple ID stays in this machine's keyring. Revoking a certificate
              only affects apps signed with it — re-sign to fix them.
            </div>
          </div>
        </div>

        {/* Main */}
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex items-end justify-between gap-6 px-8 pt-7">
            <div>
              <h1 className="text-[22px] font-semibold tracking-tight">Settings</h1>
              <p className="mt-1.5 text-[13.5px] text-slate-500 dark:text-slate-400">
                Certificates and account. Most people never need this — until Apple's
                certificate limit blocks a signing.
              </p>
            </div>
          </div>

          <div className="flex-1 min-h-0 space-y-6 overflow-y-auto px-8 py-5">
            <CertificatesSection
              signedIn={!!signedIn}
              certs={certs}
              revoking={revoking}
              onRevoke={(serial) => revoke.mutate(serial)}
              onRefresh={() => certs.refetch()}
              needs2fa={needs2fa}
              twoFa={twoFa}
              onTwoFaChange={setTwoFa}
              onVerify={() => certs.refetch()}
            />
            <AppleIdSection
              creds={creds}
              forgetting={forget.isPending}
              onForget={() => forget.mutate()}
              onSaved={() => {
                qc.invalidateQueries({ queryKey: ["cred-status"] });
                certs.refetch();
              }}
            />
          </div>

          <div className="flex items-center justify-end border-t border-slate-200 bg-slate-50/60 px-8 py-3.5 dark:border-slate-800 dark:bg-slate-950">
            <Button size="sm" onClick={onClose}>
              Done
            </Button>
          </div>
        </div>
      </div>
    </GnomeWindow>
  );
}

function CertificatesSection({
  signedIn,
  certs,
  revoking,
  onRevoke,
  onRefresh,
  needs2fa,
  twoFa,
  onTwoFaChange,
  onVerify,
}: {
  signedIn: boolean;
  certs: ReturnType<typeof useQuery<Awaited<ReturnType<typeof api.listCertificates>>>>;
  revoking: string | null;
  onRevoke: (serial: string) => void;
  onRefresh: () => void;
  needs2fa: boolean;
  twoFa: string;
  onTwoFaChange: (code: string) => void;
  onVerify: () => void;
}) {
  const list = certs.data ?? [];
  const atCap = list.length >= 2;
  const error = certs.error ? asCommandError(certs.error) : null;
  // A 2FA challenge is expected control flow, shown as its own prompt — keep it
  // out of the generic error box (mirrors ImportModal's `showError`).
  const showError = error && error.category !== "AppleAuth2FARequired";

  return (
    <section className="rounded-xl border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center gap-3 border-b border-slate-200 px-5 py-4 dark:border-slate-800">
        <Icon name="shieldCheck" size={16} className="text-slate-500" />
        <div className="flex-1">
          <div className="text-[15px] font-semibold tracking-tight">Signing certificates</div>
          <div className="mt-0.5 text-[12.5px] text-slate-500 dark:text-slate-400">
            A free Apple ID allows about 2. If signing fails at the limit, revoke an
            old one here.
          </div>
        </div>
        {signedIn && (
          <Badge tone={atCap ? "warning" : "neutral"}>
            {list.length} of ~2 used
          </Badge>
        )}
        <Button
          variant="ghost"
          size="sm"
          iconLeft="refresh"
          onClick={onRefresh}
          disabled={!signedIn || certs.isFetching}
        >
          {certs.isFetching ? "Loading…" : "Refresh"}
        </Button>
      </div>

      <div className="px-5 py-4">
        {!signedIn ? (
          <Empty>Sign in with your Apple ID below to see its certificates.</Empty>
        ) : needs2fa ? (
          <TwoFaPrompt
            twoFa={twoFa}
            onChange={onTwoFaChange}
            onVerify={onVerify}
            verifying={certs.isFetching}
          />
        ) : certs.isLoading ? (
          <Empty>Asking Apple for your certificates…</Empty>
        ) : showError ? (
          <div className="rounded-lg border border-amber-200 bg-amber-50 px-4 py-3 text-[12.5px] text-amber-800 dark:border-amber-900 dark:bg-amber-950/40 dark:text-amber-300">
            {error!.remediation}
          </div>
        ) : list.length === 0 ? (
          <Empty>No certificates yet. One is created the first time you sign an app.</Empty>
        ) : (
          <div className="space-y-2">
            {list.map((c) => {
              const busy = revoking === c.serialNumber;
              return (
                <div
                  key={c.serialNumber}
                  className="flex items-center gap-4 rounded-lg border border-slate-200 bg-white p-3.5 dark:border-slate-800 dark:bg-slate-900"
                >
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
                    <Icon name="key" size={15} />
                  </div>
                  <div className="min-w-0 flex-1">
                    <div className="text-[13.5px] font-medium">{c.name}</div>
                    <div className="mt-0.5 font-mono text-[11px] text-slate-400">
                      {c.serialNumber}
                    </div>
                  </div>
                  <Button
                    variant="outline"
                    size="sm"
                    iconLeft="trash"
                    onClick={() => onRevoke(c.serialNumber)}
                    disabled={!!revoking}
                    className="text-red-600 hover:border-red-300 hover:bg-red-50 dark:text-red-400 dark:hover:bg-red-950/40"
                  >
                    {busy ? "Revoking…" : "Revoke"}
                  </Button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </section>
  );
}

function AppleIdSection({
  creds,
  forgetting,
  onForget,
  onSaved,
}: {
  creds: ReturnType<typeof useQuery<Awaited<ReturnType<typeof api.credentialStatus>>>>;
  forgetting: boolean;
  onForget: () => void;
  onSaved: () => void;
}) {
  const mode = creds.data?.mode ?? "none";
  const keyringAvailable = creds.data?.keyringAvailable ?? true;
  const currentAppleId = creds.data?.appleId ?? null;
  const signedIn = mode !== "none";

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [remember, setRemember] = useState<RememberMode>("keyring");
  const effectiveRemember: RememberMode = keyringAvailable ? remember : "session";

  // When signed in, the form is collapsed behind "Switch account" so the screen
  // shows which account is active instead of a confusing empty login form. When
  // signed out, the form is the only thing here, so it's open.
  const [switching, setSwitching] = useState(false);
  const showForm = !signedIn || switching;

  const save = useMutation({
    mutationFn: () => api.setAppleCredentials(email, password, effectiveRemember),
    onSuccess: () => {
      setEmail("");
      setPassword("");
      setSwitching(false);
      onSaved();
    },
  });
  const saveError = save.error ? asCommandError(save.error) : null;

  const where =
    mode === "keyring"
      ? "Saved on this device"
      : mode === "session"
        ? "This session only"
        : "Not signed in";

  return (
    <section className="rounded-xl border border-slate-200 bg-white dark:border-slate-800 dark:bg-slate-900">
      <div className="flex items-center gap-3 border-b border-slate-200 px-5 py-4 dark:border-slate-800">
        <Icon name="user" size={16} className="text-slate-500" />
        <div className="flex-1">
          <div className="text-[15px] font-semibold tracking-tight">Apple ID</div>
          <div className="mt-0.5 text-[12.5px] text-slate-500 dark:text-slate-400">
            The account ReSide signs and refreshes with.
          </div>
        </div>
        <Badge tone={signedIn ? "success" : "neutral"}>{where}</Badge>
        {signedIn && (
          <Button variant="ghost" size="sm" onClick={onForget} disabled={forgetting}>
            {forgetting ? "Forgetting…" : "Forget"}
          </Button>
        )}
      </div>

      <div className="space-y-4 px-5 py-4">
        {/* Identity row: shows which account is active, so a signed-in user sees
            their Apple ID rather than a blank form that reads as "signed out". */}
        {signedIn && (
          <div className="flex items-center gap-3 rounded-lg border border-slate-200 bg-slate-50/60 px-4 py-3 dark:border-slate-800 dark:bg-slate-950">
            <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full bg-slate-100 text-slate-500 dark:bg-slate-800 dark:text-slate-400">
              <Icon name="user" size={15} />
            </div>
            <div className="min-w-0 flex-1">
              <div className="truncate text-[13.5px] font-medium">
                {currentAppleId ?? "Signed in"}
              </div>
              <div className="mt-0.5 text-[11.5px] text-slate-500 dark:text-slate-400">
                {where}
              </div>
            </div>
            {!switching && (
              <Button variant="outline" size="sm" onClick={() => setSwitching(true)}>
                Switch account
              </Button>
            )}
          </div>
        )}

        {showForm && (
          <div className="space-y-4">
            <div className="text-[12.5px] text-slate-500 dark:text-slate-400">
              {signedIn
                ? "Enter a different Apple ID to switch accounts. The current one is replaced."
                : "Sign in to sign apps. Apple may ask for a verification code the first time."}
            </div>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label>Apple ID</Label>
                <Input
                  type="email"
                  placeholder="you@icloud.com"
                  value={email}
                  onChange={(e) => setEmail(e.currentTarget.value)}
                />
              </div>
              <div className="space-y-1.5">
                <Label>Password</Label>
                <Input
                  type="password"
                  placeholder="••••••••"
                  value={password}
                  onChange={(e) => setPassword(e.currentTarget.value)}
                />
              </div>
            </div>

            <div className="flex items-center gap-2">
              <RememberPill
                active={effectiveRemember === "keyring"}
                disabled={!keyringAvailable}
                onClick={() => setRemember("keyring")}
                label="Save on this device"
                hint={keyringAvailable ? "Enables automatic refresh" : "No keyring detected"}
              />
              <RememberPill
                active={effectiveRemember === "session"}
                onClick={() => setRemember("session")}
                label="Just this session"
                hint="Cleared when ReSide closes"
              />
            </div>

            {saveError && (
              <div className="rounded-lg border border-red-200 bg-red-50 px-4 py-2.5 text-[12.5px] text-red-700 dark:border-red-900 dark:bg-red-950/40 dark:text-red-300">
                {saveError.remediation}
              </div>
            )}

            <Separator />
            <div className="flex justify-end gap-2">
              {switching && (
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => {
                    setSwitching(false);
                    setEmail("");
                    setPassword("");
                  }}
                  disabled={save.isPending}
                >
                  Cancel
                </Button>
              )}
              <Button
                size="sm"
                onClick={() => save.mutate()}
                disabled={!email || !password || save.isPending}
              >
                {save.isPending ? "Saving…" : signedIn ? "Switch account" : "Sign in"}
              </Button>
            </div>
          </div>
        )}
      </div>
    </section>
  );
}

function RememberPill({
  active,
  disabled = false,
  onClick,
  label,
  hint,
}: {
  active: boolean;
  disabled?: boolean;
  onClick: () => void;
  label: string;
  hint: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      className={cn(
        "flex-1 rounded-lg border px-3 py-2 text-left transition-colors disabled:opacity-50",
        active
          ? "border-slate-900 bg-slate-50 dark:border-slate-100 dark:bg-slate-800"
          : "border-slate-200 bg-white hover:border-slate-300 dark:border-slate-700 dark:bg-slate-900 dark:hover:border-slate-600"
      )}
    >
      <div className="text-[12.5px] font-medium">{label}</div>
      <div className="mt-0.5 text-[11px] text-slate-500 dark:text-slate-400">{hint}</div>
    </button>
  );
}

// Inline 2FA entry for cert list/revoke, mirroring ImportModal's prompt. Shown
// only when Apple challenges the login (rare — a trusted device skips it). The
// code is held by the parent and reused for follow-up cert calls.
function TwoFaPrompt({
  twoFa,
  onChange,
  onVerify,
  verifying,
}: {
  twoFa: string;
  onChange: (code: string) => void;
  onVerify: () => void;
  verifying: boolean;
}) {
  return (
    <div className="space-y-2.5 rounded-lg border border-amber-300 bg-amber-50 p-4 dark:border-amber-800 dark:bg-amber-950/30">
      <div className="flex items-center gap-2">
        <Icon name="shield" size={14} className="text-amber-700 dark:text-amber-300" />
        <div className="text-[13px] font-semibold text-amber-900 dark:text-amber-200">
          Two-factor code required
        </div>
      </div>
      <p className="text-[12px] text-amber-800 dark:text-amber-300/80">
        Apple sent a code to your trusted devices. Enter it to see your certificates.
      </p>
      <div className="flex items-center gap-2">
        <Input
          value={twoFa}
          onChange={(e) => onChange(e.currentTarget.value.replace(/\D/g, "").slice(0, 6))}
          onKeyDown={(e) => {
            if (e.key === "Enter" && twoFa.length === 6 && !verifying) onVerify();
          }}
          placeholder="123456"
          inputMode="numeric"
          className="font-mono tracking-[0.3em]"
        />
        <Button size="sm" onClick={onVerify} disabled={twoFa.length < 6 || verifying}>
          {verifying ? "Verifying…" : "Verify"}
        </Button>
      </div>
    </div>
  );
}

function Empty({ children }: { children: ReactNode }) {
  return (
    <div className="rounded-lg border border-dashed border-slate-200 px-4 py-6 text-center text-[12.5px] text-slate-500 dark:border-slate-700 dark:text-slate-400">
      {children}
    </div>
  );
}
