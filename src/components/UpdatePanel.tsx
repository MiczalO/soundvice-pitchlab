import { useState } from "react";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { toAppError } from "../lib/api";

type Phase = "idle" | "checking" | "none" | "available" | "downloading" | "error";

/** Sprawdzanie i instalacja aktualizacji (podpisane wydania z GitHub Releases). */
export function UpdatePanel({ version }: { version: string }) {
  const [phase, setPhase] = useState<Phase>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  const checkNow = async () => {
    setPhase("checking");
    setError(null);
    try {
      const u = await check();
      setUpdate(u);
      setPhase(u ? "available" : "none");
    } catch (e) {
      setError(toAppError(e).message);
      setPhase("error");
    }
  };

  const install = async () => {
    if (!update) return;
    setPhase("downloading");
    setProgress(0);
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((ev) => {
        if (ev.event === "Started") total = ev.data.contentLength ?? 0;
        else if (ev.event === "Progress") {
          done += ev.data.chunkLength;
          setProgress(total ? Math.min(100, Math.round((done / total) * 100)) : null);
        }
      });
      await relaunch();
    } catch (e) {
      setError(toAppError(e).message);
      setPhase("error");
    }
  };

  return (
    <>
      <h2>Aktualizacje</h2>
      <p className="hint">
        Wersja {version || "—"}. Aktualizacje są podpisane i pobierane z GitHub Releases; aplikacja
        uruchomi się ponownie po instalacji.
      </p>
      <div className="row">
        <button
          className="btn"
          disabled={phase === "checking" || phase === "downloading"}
          onClick={checkNow}
        >
          {phase === "checking" ? "Sprawdzam…" : "Sprawdź aktualizacje"}
        </button>
        {phase === "available" && update && (
          <button className="btn primary" onClick={install}>
            Zainstaluj {update.version} i uruchom ponownie
          </button>
        )}
      </div>
      {phase === "none" && (
        <div className="finished" role="status">
          Masz najnowszą wersję.
        </div>
      )}
      {phase === "available" && update && (
        <div className="hint" role="status">
          Dostępna wersja {update.version}.{update.body ? ` ${update.body}` : ""}
        </div>
      )}
      {phase === "downloading" && (
        <div className="hint" role="status">
          Pobieram aktualizację{progress != null ? ` (${progress}%)` : ""}…
        </div>
      )}
      {phase === "error" && (
        <div className="err-box">
          Nie udało się sprawdzić lub zainstalować aktualizacji: {error}
        </div>
      )}
    </>
  );
}
