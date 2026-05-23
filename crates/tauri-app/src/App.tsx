import { isTauri } from "./lib/ipc";
import { Gallery } from "./Gallery";
import { ReSideApp } from "./ReSideApp";

// In the Tauri window we run the live app (wired to the Rust backend); in a
// plain browser (`pnpm dev`) we show the design gallery of all screens.
export function App() {
  return isTauri() ? <ReSideApp /> : <Gallery />;
}
