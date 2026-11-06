import React from "react";
import ReactDOM from "react-dom/client";
import "./fonts";
import App from "./App";

async function boot() {
  // Podgląd w zwykłej przeglądarce (tylko DEV): udawany backend z przykładowymi danymi.
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
    await import("./dev/mockTauri");
  }
  ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
    <React.StrictMode>
      <App />
    </React.StrictMode>,
  );
}

void boot();
