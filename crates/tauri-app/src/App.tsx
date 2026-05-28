import { isTauri } from "./lib/ipc";
import { Gallery } from "./Gallery";
import { ReSideApp } from "./ReSideApp";

// In the Tauri window we run the live app — `ReSideApp`, wired to the Rust
// backend through the IPC layer. In a plain browser (`pnpm dev`) we instead
// show `Gallery`, a design-preview harness that renders the screen artboards
// with **mock data** for layout review. Browser mode never reaches the Rust
// backend; nothing on screen there is real.
export function App() {
  return isTauri() ? <ReSideApp /> : <Gallery />;
}
