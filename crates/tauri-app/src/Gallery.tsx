// Design gallery: a screen-picker harness that renders every artboard at its
// design size with mock data. Shown when running in a plain browser
// (`pnpm dev`); the live app (`Tauri`) renders ReSideApp instead.

import { useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Setup } from "./screens/Setup";
import { Pairing } from "./screens/Pairing";
import { Dashboard } from "./screens/Dashboard";
import { Import } from "./screens/Import";
import { Install } from "./screens/Install";
import { Tray } from "./screens/Tray";
import { Icon } from "./components/ui";
import { cn } from "./lib/cn";

const W = 1180;
const H = 760;

interface Artboard {
  id: string;
  group: string;
  label: string;
  w: number;
  h: number;
  render: (dark: boolean) => ReactNode;
}

const ARTBOARDS: Artboard[] = [
  { id: "setup", group: "① First-run setup", label: "System check · 4 of 6 ready", w: W, h: H, render: (d) => <Setup dark={d} /> },
  { id: "pairing", group: "② Device pairing", label: "Waiting for trust prompt", w: W, h: H, render: (d) => <Pairing dark={d} /> },
  { id: "dashboard", group: "③ Main dashboard", label: "6 installed apps · varied statuses", w: W, h: H, render: (d) => <Dashboard dark={d} /> },
  { id: "dashboard-empty", group: "③ Main dashboard", label: "Empty state · first launch", w: W, h: H, render: (d) => <Dashboard dark={d} empty /> },
  { id: "import", group: "④ Import IPA & sign", label: "Free Apple ID · ready to install", w: W, h: H, render: (d) => <Import dark={d} /> },
  { id: "import-error", group: "④ Import IPA & sign", label: "Error · 2FA required", w: W, h: H, render: (d) => <Import dark={d} errorState /> },
  { id: "install", group: "⑤ Install progress", label: "Transferring · 62%", w: W, h: H, render: (d) => <Install dark={d} /> },
  { id: "install-error", group: "⑤ Install progress", label: "Error · device is locked", w: W, h: H, render: (d) => <Install dark={d} errorState /> },
  { id: "tray", group: "⑥ Tray popover", label: "Tray popover · 360 × 500", w: 360, h: 500, render: (d) => <Tray dark={d} /> },
];

export function Gallery() {
  const [selectedId, setSelectedId] = useState(ARTBOARDS[0].id);
  const [dark, setDark] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const [scale, setScale] = useState(1);

  const active = ARTBOARDS.find((a) => a.id === selectedId) ?? ARTBOARDS[0];

  // Scale the artboard down to fit the stage area while preserving aspect.
  useLayoutEffect(() => {
    const el = stageRef.current;
    if (!el) return;
    const fit = () => {
      const pad = 64;
      const sx = (el.clientWidth - pad) / active.w;
      const sy = (el.clientHeight - pad) / active.h;
      setScale(Math.min(1, sx, sy));
    };
    fit();
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    return () => ro.disconnect();
  }, [active.w, active.h]);

  // Group artboards for the rail.
  const groups: Array<{ name: string; items: Artboard[] }> = [];
  for (const a of ARTBOARDS) {
    const g = groups.find((x) => x.name === a.group);
    if (g) g.items.push(a);
    else groups.push({ name: a.group, items: [a] });
  }

  return (
    <div className="flex h-screen w-screen overflow-hidden bg-[#f0eee9] font-sans text-slate-900">
      {/* Picker rail */}
      <aside className="flex w-[280px] shrink-0 flex-col border-r border-slate-300/70 bg-white">
        <div className="flex items-center gap-2 border-b border-slate-200 px-4 py-3.5">
          <div className="flex h-7 w-7 items-center justify-center rounded-md bg-slate-900 text-slate-50">
            <Icon name="refresh" size={14} strokeWidth={2.25} />
          </div>
          <div>
            <div className="text-[14px] font-semibold tracking-tight">ReSide</div>
            <div className="text-[10.5px] text-slate-500">Design gallery · 6 flows</div>
          </div>
        </div>

        <div className="flex-1 overflow-y-auto px-3 py-3">
          {groups.map((g) => (
            <div key={g.name} className="mb-3">
              <div className="px-2 pb-1 text-[10.5px] font-semibold uppercase tracking-wider text-slate-400">
                {g.name}
              </div>
              {g.items.map((a) => (
                <button
                  key={a.id}
                  onClick={() => setSelectedId(a.id)}
                  className={cn(
                    "mb-0.5 flex w-full items-center gap-2 rounded-md px-2.5 py-1.5 text-left text-[12.5px] transition-colors",
                    a.id === selectedId
                      ? "bg-slate-900 text-slate-50"
                      : "text-slate-600 hover:bg-slate-100 hover:text-slate-900"
                  )}
                >
                  <span className="truncate">{a.label}</span>
                </button>
              ))}
            </div>
          ))}
        </div>

        {/* Theme toggle */}
        <div className="border-t border-slate-200 px-4 py-3">
          <button
            onClick={() => setDark((v) => !v)}
            className="flex w-full items-center justify-between rounded-md border border-slate-200 px-3 py-2 text-[12.5px] font-medium text-slate-700 transition-colors hover:bg-slate-50"
          >
            <span className="flex items-center gap-2">
              <Icon name={dark ? "moon" : "sun"} size={14} />
              {dark ? "Dark mode" : "Light mode"}
            </span>
            <span
              className={cn(
                "relative h-5 w-9 rounded-full transition-colors",
                dark ? "bg-slate-900" : "bg-slate-200"
              )}
            >
              <span
                className={cn(
                  "absolute top-0.5 h-4 w-4 rounded-full bg-white shadow transition-all",
                  dark ? "left-[18px]" : "left-0.5"
                )}
              />
            </span>
          </button>
        </div>
      </aside>

      {/* Stage */}
      <main
        ref={stageRef}
        className="flex flex-1 items-center justify-center overflow-hidden transition-colors"
        style={{ background: dark ? "#0b1220" : "#f0eee9" }}
      >
        <div style={{ width: active.w * scale, height: active.h * scale }}>
          <div
            style={{
              width: active.w,
              height: active.h,
              transform: `scale(${scale})`,
              transformOrigin: "top left",
            }}
          >
            {active.render(dark)}
          </div>
        </div>
      </main>
    </div>
  );
}
