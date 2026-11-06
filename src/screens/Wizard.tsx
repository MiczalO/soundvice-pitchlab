import { useEffect, useRef, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Brand } from "../components/Brand";
import { modelLabel, useModels } from "../components/Models";
import { api, toAppError } from "../lib/api";
import { useApp } from "../lib/store";

export function Wizard() {
  const { settings, hasGemini, saveSetting, refreshKeys, notify } = useApp();
  const { models, status, load, loading, error } = useModels();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  const [limited, setLimited] = useState(false);
  // Klucz już w sejfie (np. po przerwanym kreatorze) → od razu krok 2, bez ponownego wpisywania.
  const step = hasGemini ? 2 : 1;
  const autoProbed = useRef(false);

  useEffect(() => {
    if (hasGemini && !autoProbed.current && Object.keys(status).length === 0) {
      autoProbed.current = true;
      void load(true);
    }
  }, [hasGemini, status, load]);

  const connect = async () => {
    setBusy(true);
    setErr(null);
    try {
      await api.setSecret("gemini", key);
      setKey("");
      autoProbed.current = true;
      const list = await load(true);
      if (!list.length) throw new Error("Nie udało się pobrać listy modeli — sprawdź klucz.");
      await refreshKeys();
    } catch (e) {
      await api.deleteSecret("gemini").catch(() => {});
      await refreshKeys();
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
      // Klucz jest poprawny (lista modeli się pobrała) — problem dotyczy tylko wybranego modelu.
      setLimited(["rate_limited", "overloaded", "timeout"].includes(ae.code));
    } finally {
      setBusy(false);
    }
  };

  const startAnyway = async () => {
    await saveSetting("onboarded", "1");
    notify("Klucz działa. Zmień model w Ustawieniach, jeśli ten nie odpowiada.");
  };

  const modelSelect = (id: string, key: string, label: string) => (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <select
        id={id}
        value={settings[key] ?? ""}
        onChange={(e) => saveSetting(key, e.target.value)}
        disabled={loading}
      >
        {models.map((m) => (
          <option key={m} value={m}>
            {modelLabel(m, status)}
          </option>
        ))}
      </select>
    </div>
  );

  const working = models.filter((m) => status[m] === "ok").length;

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
                {busy || loading ? "Łączę i sprawdzam modele…" : "Połącz i pobierz modele"}
              </button>
            </div>
            {(busy || loading) && (
              <span className="hint">
                Sprawdzam, które modele naprawdę odpowiadają dla Twojego klucza (do 30 s).
              </span>
            )}
          </div>
        ) : (
          <div className="panel">
            {loading ? (
              <p className="hint">
                Sprawdzam, które modele naprawdę odpowiadają dla Twojego klucza (do 30 s)…
              </p>
            ) : (
              <>
                {modelSelect("wModel", "gemini_model", "Model do odsłuchu i analizy")}
                {modelSelect(
                  "wModelR",
                  "gemini_model_research",
                  "Model do researchu (Google Search)",
                )}
                <span className="hint">
                  {working
                    ? `Działających modeli: ${working}. Wybrany został pierwszy z nich.`
                    : "Żaden model nie odpowiedział poprawnie — spróbuj ponownie za chwilę."}
                </span>
              </>
            )}
            {(err || error) && <div className="err-box">{err || error}</div>}
            <div className="row">
              <button className="btn primary big" disabled={busy || loading} onClick={finish}>
                {busy ? "Sprawdzam…" : "Sprawdź i zacznij"}
              </button>
              <button className="btn" disabled={busy || loading} onClick={() => load(true)}>
                Sprawdź modele ponownie
              </button>
              {limited && (
                <button className="btn" disabled={busy || loading} onClick={startAnyway}>
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
