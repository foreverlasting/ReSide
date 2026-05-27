import React from "react";
import ReactDOM from "react-dom/client";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { App } from "./App";
import "./index.css";
// Palette overrides layered on top of Tailwind's slate utilities — dark mode
// becomes Dracula, light mode Catppuccin Latte. Each keys off the
// `data-theme` attribute the GnomeWindow sets, so they only repaint in-app
// surfaces. Imported after index.css so they win the cascade.
import "./themes/dracula.css";
import "./themes/catppuccin-latte.css";

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false, refetchOnWindowFocus: false } },
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <QueryClientProvider client={queryClient}>
      <App />
    </QueryClientProvider>
  </React.StrictMode>
);
