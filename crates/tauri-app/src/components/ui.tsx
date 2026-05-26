import type { ReactNode, ButtonHTMLAttributes, InputHTMLAttributes, HTMLAttributes } from "react";
import { cn } from "../lib/cn";
import { Icon, type IconName } from "./icon";

// -------------------- Surfaces --------------------
export const Card = ({ className = "", children, ...rest }: HTMLAttributes<HTMLDivElement>) => (
  <div
    className={cn(
      "rounded-xl border border-slate-200 bg-white",
      "dark:border-slate-800 dark:bg-slate-900",
      className
    )}
    {...rest}
  >
    {children}
  </div>
);

export const CardHeader = ({ className = "", children }: { className?: string; children?: ReactNode }) => (
  <div className={cn("px-5 pt-5", className)}>{children}</div>
);
export const CardTitle = ({ className = "", children }: { className?: string; children?: ReactNode }) => (
  <div className={cn("text-[15px] font-semibold tracking-tight text-slate-900 dark:text-slate-100", className)}>
    {children}
  </div>
);
export const CardDescription = ({ className = "", children }: { className?: string; children?: ReactNode }) => (
  <div className={cn("text-[13px] text-slate-500 dark:text-slate-400 mt-1", className)}>{children}</div>
);
export const CardContent = ({ className = "", children }: { className?: string; children?: ReactNode }) => (
  <div className={cn("px-5 py-5", className)}>{children}</div>
);

// -------------------- Buttons --------------------
type ButtonVariant = "default" | "secondary" | "outline" | "ghost" | "destructive" | "link";
type ButtonSize = "sm" | "md" | "lg" | "icon";

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: ButtonVariant;
  size?: ButtonSize;
  iconLeft?: IconName;
  iconRight?: IconName;
}

export const Button = ({
  variant = "default",
  size = "md",
  className = "",
  children,
  iconLeft,
  iconRight,
  ...rest
}: ButtonProps) => {
  const base =
    "inline-flex items-center justify-center gap-2 font-medium whitespace-nowrap " +
    "rounded-md transition-colors select-none " +
    "focus:outline-none focus-visible:ring-2 focus-visible:ring-offset-2 " +
    "focus-visible:ring-slate-900 focus-visible:ring-offset-white " +
    "dark:focus-visible:ring-slate-100 dark:focus-visible:ring-offset-slate-950 " +
    "disabled:opacity-50 disabled:pointer-events-none";

  const variants: Record<ButtonVariant, string> = {
    default:
      "bg-slate-900 text-slate-50 hover:bg-slate-800 active:bg-slate-950 " +
      "dark:bg-slate-50 dark:text-slate-900 dark:hover:bg-slate-200",
    secondary:
      "bg-slate-100 text-slate-900 hover:bg-slate-200 " +
      "dark:bg-slate-800 dark:text-slate-100 dark:hover:bg-slate-700",
    outline:
      "border border-slate-200 bg-white text-slate-900 hover:bg-slate-50 hover:border-slate-300 " +
      "dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100 dark:hover:bg-slate-800",
    ghost:
      "text-slate-700 hover:bg-slate-100 hover:text-slate-900 " +
      "dark:text-slate-300 dark:hover:bg-slate-800 dark:hover:text-slate-100",
    destructive: "bg-red-600 text-white hover:bg-red-700 dark:bg-red-600 dark:hover:bg-red-700",
    link: "text-slate-900 underline-offset-4 hover:underline dark:text-slate-100",
  };

  const sizes: Record<ButtonSize, string> = {
    sm: "h-8 px-3 text-[13px]",
    md: "h-9 px-3.5 text-[13px]",
    lg: "h-10 px-4 text-sm",
    icon: "h-9 w-9 p-0",
  };

  return (
    <button className={cn(base, variants[variant], sizes[size], className)} {...rest}>
      {iconLeft && <Icon name={iconLeft} size={size === "lg" ? 16 : 14} />}
      {children}
      {iconRight && <Icon name={iconRight} size={size === "lg" ? 16 : 14} />}
    </button>
  );
};

// -------------------- Badge --------------------
type BadgeTone = "neutral" | "success" | "warning" | "danger" | "info" | "solid";

export const Badge = ({
  tone = "neutral",
  className = "",
  children,
}: {
  tone?: BadgeTone;
  className?: string;
  children?: ReactNode;
}) => {
  const tones: Record<BadgeTone, string> = {
    neutral:
      "bg-slate-100 text-slate-700 border-slate-200 dark:bg-slate-800 dark:text-slate-300 dark:border-slate-700",
    success:
      "bg-emerald-50 text-emerald-700 border-emerald-200 dark:bg-emerald-950/40 dark:text-emerald-300 dark:border-emerald-900",
    warning:
      "bg-amber-50 text-amber-800 border-amber-200 dark:bg-amber-950/40 dark:text-amber-300 dark:border-amber-900",
    danger:
      "bg-red-50 text-red-700 border-red-200 dark:bg-red-950/40 dark:text-red-300 dark:border-red-900",
    info: "bg-sky-50 text-sky-700 border-sky-200 dark:bg-sky-950/40 dark:text-sky-300 dark:border-sky-900",
    solid:
      "bg-slate-900 text-slate-50 border-slate-900 dark:bg-slate-100 dark:text-slate-900 dark:border-slate-100",
  };
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium",
        tones[tone],
        className
      )}
    >
      {children}
    </span>
  );
};

// -------------------- Form controls --------------------
export const Input = ({ className = "", ...rest }: InputHTMLAttributes<HTMLInputElement>) => (
  <input
    className={cn(
      "h-9 w-full rounded-md border border-slate-200 bg-white px-3 text-[13px] text-slate-900 placeholder:text-slate-400",
      "focus:outline-none focus:ring-2 focus:ring-slate-900/20 focus:border-slate-400",
      "dark:border-slate-700 dark:bg-slate-900 dark:text-slate-100 dark:placeholder:text-slate-500",
      "dark:focus:ring-slate-100/20 dark:focus:border-slate-500",
      className
    )}
    {...rest}
  />
);

export const Label = ({
  className = "",
  children,
  ...rest
}: HTMLAttributes<HTMLLabelElement>) => (
  <label className={cn("text-[12px] font-medium text-slate-700 dark:text-slate-300", className)} {...rest}>
    {children}
  </label>
);

export const Checkbox = ({
  checked = false,
  className = "",
  onChange = () => {},
}: {
  checked?: boolean;
  className?: string;
  onChange?: (v: boolean) => void;
}) => (
  <button
    type="button"
    role="checkbox"
    aria-checked={checked}
    onClick={() => onChange(!checked)}
    className={cn(
      "h-4 w-4 rounded border flex items-center justify-center transition-colors",
      checked
        ? "bg-slate-900 border-slate-900 dark:bg-slate-100 dark:border-slate-100"
        : "border-slate-300 bg-white dark:border-slate-600 dark:bg-slate-900",
      className
    )}
  >
    {checked && <Icon name="check" size={11} className="text-white dark:text-slate-900" strokeWidth={3} />}
  </button>
);

export const Switch = ({
  checked = false,
  onChange = () => {},
  className = "",
}: {
  checked?: boolean;
  onChange?: (v: boolean) => void;
  className?: string;
}) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    onClick={() => onChange(!checked)}
    className={cn(
      "h-5 w-9 rounded-full transition-colors relative",
      checked ? "bg-slate-900 dark:bg-slate-100" : "bg-slate-200 dark:bg-slate-700",
      className
    )}
  >
    <span
      className={cn(
        "absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all dark:bg-slate-900",
        checked ? "left-[18px]" : "left-0.5"
      )}
    />
  </button>
);

// -------------------- Progress --------------------
// `indeterminate` shows a sliding "working" bar instead of a fixed width — for
// steps whose duration we can't measure (the signer is a black box), so the bar
// reads as active rather than frozen at an arbitrary percentage.
export const Progress = ({
  value = 0,
  indeterminate = false,
  className = "",
}: {
  value?: number;
  indeterminate?: boolean;
  className?: string;
}) => (
  <div className={cn("h-1.5 w-full overflow-hidden rounded-full bg-slate-100 dark:bg-slate-800", className)}>
    {indeterminate ? (
      <div
        className="h-full w-1/4 rounded-full bg-slate-900 dark:bg-slate-100"
        style={{ animation: "indeterminate-slide 1.1s ease-in-out infinite" }}
      />
    ) : (
      <div
        className="h-full rounded-full bg-slate-900 dark:bg-slate-100 transition-all"
        style={{ width: `${Math.max(0, Math.min(100, value))}%` }}
      />
    )}
  </div>
);

// -------------------- Separator --------------------
export const Separator = ({ className = "", vertical = false }: { className?: string; vertical?: boolean }) => (
  <div
    className={cn("bg-slate-200 dark:bg-slate-800", vertical ? "w-px h-full" : "h-px w-full", className)}
  />
);

// -------------------- Kbd --------------------
export const Kbd = ({ children, className = "" }: { children?: ReactNode; className?: string }) => (
  <kbd
    className={cn(
      "inline-flex h-5 min-w-5 items-center justify-center rounded border px-1.5",
      "border-slate-200 bg-slate-50 text-[10.5px] font-mono text-slate-600",
      "dark:border-slate-700 dark:bg-slate-900 dark:text-slate-400",
      className
    )}
  >
    {children}
  </kbd>
);

// -------------------- AppTile --------------------
// Stand-in for an iOS app icon: a gradient + the name's initial.
type TileColor =
  | "slate" | "blue" | "green" | "violet" | "orange" | "pink" | "teal" | "indigo" | "amber" | "red";

export const AppTile = ({
  name = "?",
  color = "slate",
  size = 40,
  className = "",
  rounded = "rounded-[22%]",
}: {
  name?: string;
  color?: TileColor;
  size?: number;
  className?: string;
  rounded?: string;
}) => {
  const palettes: Record<TileColor, [string, string]> = {
    slate: ["#475569", "#0f172a"],
    blue: ["#3b82f6", "#1d4ed8"],
    green: ["#10b981", "#047857"],
    violet: ["#8b5cf6", "#6d28d9"],
    orange: ["#f97316", "#c2410c"],
    pink: ["#ec4899", "#be185d"],
    teal: ["#14b8a6", "#0f766e"],
    indigo: ["#6366f1", "#4338ca"],
    amber: ["#f59e0b", "#b45309"],
    red: ["#ef4444", "#b91c1c"],
  };
  const [from, to] = palettes[color] || palettes.slate;
  const initial = String(name).trim()[0]?.toUpperCase() || "?";
  return (
    <div
      className={cn("flex items-center justify-center font-semibold text-white shrink-0", rounded, className)}
      style={{
        width: size,
        height: size,
        background: `linear-gradient(135deg, ${from} 0%, ${to} 100%)`,
        fontSize: Math.round(size * 0.42),
        letterSpacing: -0.5,
        boxShadow: "inset 0 1px 0 rgba(255,255,255,.2), 0 1px 2px rgba(0,0,0,.08)",
      }}
    >
      {initial}
    </div>
  );
};

// -------------------- StatusDot --------------------
type DotTone = "neutral" | "success" | "warning" | "danger" | "info";

export const StatusDot = ({
  tone = "neutral",
  pulse = false,
  className = "",
}: {
  tone?: DotTone;
  pulse?: boolean;
  className?: string;
}) => {
  const tones: Record<DotTone, string> = {
    neutral: "bg-slate-400",
    success: "bg-emerald-500",
    warning: "bg-amber-500",
    danger: "bg-red-500",
    info: "bg-sky-500",
  };
  return (
    <span className={cn("relative inline-flex h-2 w-2 rounded-full", tones[tone], className)}>
      {pulse && <span className={cn("absolute inset-0 rounded-full animate-ping opacity-60", tones[tone])} />}
    </span>
  );
};

export { Icon } from "./icon";
export type { IconName } from "./icon";
export { cn } from "../lib/cn";
