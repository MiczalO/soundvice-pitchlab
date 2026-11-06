import { useState } from "react";
import { join } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";
import { revealItemInDir } from "@tauri-apps/plugin-opener";
import { api, toAppError, type ImportReport } from "../lib/api";
import { fmtSize } from "../lib/format";
import { useApp } from "../lib/store";

/** Kopia bazy, przywracanie, import z wersji HTML i folder danych. Klucze API nie są w kopii. */
export function DataPanel() {
  const notify = useApp((s) => s.notify);
  const [withAudio, setWithAudio] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirmRestore, setConfirmRestore] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const [error, setError] = useState<string | null>(null);

  const guard = async (name: string, fn: () => Promise<void>) => {
    setBusy(name);
    setError(null);
    try {
      await fn();
    } catch (e) {
      setError(toAppError(e).message);
    } finally {
      setBusy(null);
    }
  };

  const exportBackup = () =>
    guard("export", async () => {
      const date = new Date().toISOString().slice(0, 10);
      const path = await save({
        defaultPath: `soundvice-pitchlab-${date}.zip`,
        filters: [{ name: "Kopia pitchLAB", extensions: ["zip"] }],
      });
      if (!path) return;
      const info = await api.exportBackup(path, withAudio);
      notify(
        `Zapisano kopię (${fmtSize(info.size)}${info.audio_files ? `, ${info.audio_files} plików audio` : ""})`,
      );
    });

  const restore = () =>
    guard("restore", async () => {
      setConfirmRestore(false);
      const path = await open({
        multiple: false,
        filters: [{ name: "Kopia pitchLAB", extensions: ["zip"] }],
      });
      if (typeof path !== "string") return;
      await api.restoreBackup(path); // aplikacja się zrestartuje
    });

  const importHtml = () =>
    guard("import", async () => {
      const path = await open({
        multiple: false,
        filters: [{ name: "Kopia JSON z wersji HTML", extensions: ["json"] }],
      });
      if (typeof path !== "string") return;
      setReport(await api.importHtmlJson(path));
    });

  const revealData = () =>
    guard("folder", async () => {
      const dir = await api.getDataDir();
      await revealItemInDir(await join(dir, "pitchlab.sqlite3"));
    });

  return (
    <>
      <h2>Kopia bazy i dane</h2>
      <p className="hint">
        Kopia to jeden plik ZIP z artystami, pigułkami, pitchami i zasadami. Klucze API nie wchodzą
        do kopii — po przeniesieniu na inny komputer wpisz je od nowa.
      </p>
      <label className="check">
        <input
          type="checkbox"
          checked={withAudio}
          onChange={(e) => setWithAudio(e.target.checked)}
        />
        <span>
          Dołącz pliki audio
          <br />
          <span className="hint">
            Kopia będzie znacznie większa (mastery to dziesiątki MB każdy).
          </span>
        </span>
      </label>
      <div className="row">
        <button className="btn" disabled={busy !== null} onClick={exportBackup}>
          {busy === "export" ? "Zapisuję…" : "Eksportuj kopię"}
        </button>
        {!confirmRestore ? (
          <button className="btn" disabled={busy !== null} onClick={() => setConfirmRestore(true)}>
            Przywróć z kopii
          </button>
        ) : (
          <>
            <button className="btn danger" disabled={busy !== null} onClick={restore}>
              Wybierz kopię i zrestartuj
            </button>
            <button className="btn ghost" onClick={() => setConfirmRestore(false)}>
              Anuluj
            </button>
          </>
        )}
        <button className="btn ghost" disabled={busy !== null} onClick={revealData}>
          Pokaż folder danych
        </button>
      </div>
      {confirmRestore && (
        <p className="hint">
          Przywrócenie zastąpi obecne dane danymi z kopii i uruchomi aplikację ponownie. Poprzednia
          baza zostanie zachowana w folderze danych (before_restore).
        </p>
      )}

      <div className="sep" />
      <h3>Import z wersji HTML</h3>
      <p className="hint">
        Wczytaj plik JSON wyeksportowany przez prototyp (soundvice-pitchlab.html). Import niczego
        nie nadpisuje: istniejący artysta zachowuje swoją pigułkę, a inna pigułka z pliku trafia
        jako propozycja do porównania. Ponowny import tego samego pliku nie dubluje utworów.
      </p>
      <div className="row">
        <button className="btn" disabled={busy !== null} onClick={importHtml}>
          {busy === "import" ? "Importuję…" : "Importuj JSON"}
        </button>
      </div>
      {report && (
        <div className="finished" role="status">
          Zaimportowano: artyści {report.artists_new} nowych i {report.artists_merged} połączonych,
          utwory {report.tracks} (pominięte {report.tracks_skipped}), wersje pitchy {report.pitches}
          , propozycje pigułek {report.proposals}
          {report.rules_updated ? ", zasady zaktualizowane" : ""}
          {report.examples_updated ? ", wzorce zaktualizowane" : ""}.
        </div>
      )}
      {error && <div className="err-box">{error}</div>}
    </>
  );
}
