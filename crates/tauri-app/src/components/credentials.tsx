// Shared Apple ID credential entry: the email/password fields and the
// "remember my credentials" tier chooser. Used by both the install flow
// (ImportModal) and account management (Settings).
//
// Before this, the two screens each had their own implementation — radio rows
// vs. pills, with drifting copy and different option sets — for the same
// decision. This is the single source of truth (ROADMAP §7c). The chooser is
// tier-configurable because the contexts legitimately differ: the install flow
// offers a third "don't remember" tier (a one-shot, discarded on close), while
// account management is deliberately persisting an account and omits it.

import { Input, Label, Icon, cn } from "./ui";
import type { RememberMode } from "../lib/ipc";

export type RememberChoice = "keyring" | "session" | "ask";

// The backend persists only two tiers. "ask" has no durable store — it rides the
// in-memory session store and the caller clears it on close — so it maps to
// "session" here, as does "session" itself.
export function toRememberMode(choice: RememberChoice): RememberMode {
  return choice === "keyring" ? "keyring" : "session";
}

interface Tier {
  title: string;
  /** Copy may depend on whether a system keyring is present. */
  desc: (keyringAvailable: boolean) => string;
  /** The keyring tier is disabled when no Secret Service is available. */
  needsKeyring?: boolean;
}

const TIERS: Record<RememberChoice, Tier> = {
  keyring: {
    title: "On this device",
    desc: (ok) =>
      ok
        ? "Saved securely in your keyring · enables automatic refresh"
        : "Unavailable — install a system keyring (gnome-keyring or KWallet)",
    needsKeyring: true,
  },
  session: {
    title: "Just this session",
    desc: () => "Kept in memory until you quit ReSide; never written to disk",
  },
  ask: {
    title: "Don't remember",
    desc: () => "Ask me every time I install or refresh",
  },
};

/// The Apple ID + password pair. Stateless: the parent owns the values.
export function AppleIdFields({
  appleId,
  password,
  onAppleId,
  onPassword,
}: {
  appleId: string;
  password: string;
  onAppleId: (v: string) => void;
  onPassword: (v: string) => void;
}) {
  return (
    <div className="grid grid-cols-2 gap-3">
      <div className="space-y-1.5">
        <Label>Apple ID</Label>
        <Input
          type="email"
          value={appleId}
          onChange={(e) => onAppleId(e.currentTarget.value)}
          placeholder="you@icloud.com"
          autoComplete="off"
        />
      </div>
      <div className="space-y-1.5">
        <Label>Password</Label>
        <Input
          type="password"
          value={password}
          onChange={(e) => onPassword(e.currentTarget.value)}
          placeholder="••••••••"
          autoComplete="off"
        />
      </div>
    </div>
  );
}

/// The remember-tier chooser. `tiers` selects which options to show (and their
/// order); the keyring tier auto-disables when `keyringAvailable` is false.
export function RememberChoiceField({
  value,
  onChange,
  keyringAvailable,
  tiers = ["keyring", "session", "ask"],
  legend = "Remember my Apple ID?",
}: {
  value: RememberChoice;
  onChange: (choice: RememberChoice) => void;
  keyringAvailable: boolean;
  tiers?: RememberChoice[];
  legend?: string;
}) {
  return (
    <fieldset className="space-y-1.5">
      <legend className="mb-1 text-[12px] font-medium text-slate-700 dark:text-slate-300">
        {legend}
      </legend>
      {tiers.map((id) => {
        const tier = TIERS[id];
        const disabled = !!tier.needsKeyring && !keyringAvailable;
        return (
          <RememberOption
            key={id}
            checked={value === id}
            disabled={disabled}
            onSelect={() => onChange(id)}
            title={tier.title}
            desc={tier.desc(keyringAvailable)}
          />
        );
      })}
    </fieldset>
  );
}

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

/// The reassurance line shown under the credential form.
export function ApplePasswordNote() {
  return (
    <div className="flex items-center gap-1.5 text-[11px] text-slate-500">
      <Icon name="shieldCheck" size={12} />
      Your password is only ever sent to Apple.
    </div>
  );
}
