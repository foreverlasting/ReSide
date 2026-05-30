// System pane: a persistent home for the dependency check that, before §7h, was
// only reachable during onboarding (the Dashboard's inline "Get Started" check).
// Once onboarding went green there was no way back to it (ROADMAP §7d gap); a
// standing "System" nav item now restores it.
//
// It is a *pane*: ReSideApp renders it inside the persistent Dashboard shell's
// `<main>` (ROADMAP §7h), so no window chrome or sidebar of its own. It reuses
// the same `InlineSystemCheck` rows the onboarding panel shows — driven by the
// `setup` query ReSideApp already owns, handed in as `GetStartedHandlers`.

import { Button } from "../components/ui";
import { InlineSystemCheck, type GetStartedHandlers } from "./Dashboard";

export function System({ gs }: { gs: GetStartedHandlers }) {
  return (
    <>
      <div className="flex items-end justify-between gap-6 px-8 pt-7">
        <div>
          <h1 className="text-[22px] font-semibold tracking-tight">System</h1>
          <p className="mt-1.5 text-[13.5px] text-slate-500 dark:text-slate-400">
            The dependencies ReSide needs to sign, install, and auto-refresh. Re-run
            this any time something stops working.
          </p>
        </div>
        <Button
          variant="ghost"
          size="sm"
          iconLeft="refresh"
          disabled={gs.rerunning}
          onClick={gs.onRunCheck}
        >
          {gs.rerunning ? "Re-running…" : "Re-run check"}
        </Button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto px-8 py-5">
        <InlineSystemCheck gs={gs} inset={false} />
      </div>
    </>
  );
}
