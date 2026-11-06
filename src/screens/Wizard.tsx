import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Brand } from "../components/Brand";
import { useModels } from "../components/Models";
import { api, toAppError } from "../lib/api";
import { useApp } from "../lib/store";

export function Wizard() {
  const { settings, saveSetting, refreshKeys, notify } = useApp();
  const { models, load, loading, error } = useModels();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [step, setStep] = useState<1 | 2>(1);
  const [limited, setLimited] = useState(false);

  const connect = async () => {
    setBusy(true);
    setErr(null);
    try {
      await api.setSecret("gemini", key);
      setKey("");
      const list = await load();
      if (!list.length) throw new Error("Nie udało się pobrać listy modeli — sprawdź klucz.");
      await refreshKeys();
      setStep(2);
    } catch (e) {
      await api.deleteSecret("gemini").catch(() => {});
      setErr(toAppError(e).message);
    } finally {
      setBusy(false);
    }
  };

  const finish = async () => {
    setBusy(true);
    setErr(null);
    try {
      const r = await api.testConnection({
        geminiModel: settings.gemini_model,
        checkClaude: false,
      });
      await saveSetting("onboarded", "1");
      notify(`Połączono z Gemini (${r.gemini ?? "OK"})`);
    } catch (e) {
      const ae = toAppError(e);
      setErr(ae.message);
      // Klucz jest poprawny (lista modeli się pobrała) — limit dotyczy tylko wybranego modelu.
      setLimited(ae.code === "rate_limited");
    } finally {
      setBusy(false);
    }
  };

  const startAnyway = async () => {
    await saveSetting("onboarded", "1");
    notify("Klucz działa. Zmień model w Ustawieniach, jeśli ten ma wyczerpany limit.");
  };

  return (
    <main className="wrap" style={{ maxWidth: 640, paddingTop: 48 }}>
      <Brand />
      <section className="view">
        <div className="view-head">
          <div className="eyebrow">Pierwsze uruchomienie</div>
          <h1>Połącz Gemini</h1>
          <p className="lede">
            pitchLAB korzysta z Twojego własnego klucza API Google. Klucz trafia do sejfu
            systemowego i nigdy nie jest zapisywany w bazie ani w plikach.
          </p>
        </div>
        {step === 1 ? (
          <div className="panel">
            <div className="field">
              <label htmlFor="wKey">Klucz API Gemini</label>
              <input
                id="wKey"
                type="password"
                autoComplete="off"
                placeholder="AIza…"
                value={key}
                onChange={(e) => setKey(e.target.value)}
              />
              <span className="hint">
                Wygeneruj go w{" "}
                <a
                  href="https://aistudio.google.com/apikey"
                  onClick={(e) => {
                    e.preventDefault();
                    openUrl("https://aistudio.google.com/apikey");
                  }}
                >
                  aistudio.google.com/apikey
                </a>
                .
              </span>
            </div>
            <div className="framed-warn">
              Na darmowym planie Gemini Google może wykorzystywać przesłane dane. Przy niewydanych
              masterach zalecany jest płatny plan.
            </div>
            {(err || error) && <div className="err-box">{err || error}</div>}
            <div className="row">
              <button
                className="btn primary big"
                disabled={busy || loading || !key.trim()}
                onClick={connect}
              >
                {busy || loading ? "Łączę…" : "Połącz i pobierz modele"}
              </button>
            </div>
          </div>
        ) : (
          <div className="panel">
            <div className="field">
              <label htmlFor="wModel">Model do odsłuchu i analizy</label>
              <select
                id="wModel"
                value={settings.gemini_model ?? ""}
                onChange={(e) => saveSetting("gemini_model", e.target.value)}
              >
                {models.map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </select>
            </div>
            <div className="field">
              <label htmlFor="wModelR">Model do researchu (Google Search)</label>
              <select
                id="wModelR"
                value={settings.gemini_model_research ?? ""}
                onChange={(e) => saveSetting("gemini_model_research", e.target.value)}
              >
                {models.map((m) => (
                  <option key={m}>{m}</option>
                ))}
              </select>
              <span className="hint">Domyślnie wybrany jest najnowszy model „pro”.</span>
            </div>
            {err && <div className="err-box">{err}</div>}
            <div className="row">
              <button className="btn primary big" disabled={busy} onClick={finish}>
                {busy ? "Sprawdzam…" : "Sprawdź i zacznij"}
              </button>
              {limited && (
                <button className="btn" disabled={busy} onClick={startAnyway}>
                  Zacznij mimo to
                </button>
              )}
            </div>
          </div>
        )}
      </section>
    </main>
  );
}
