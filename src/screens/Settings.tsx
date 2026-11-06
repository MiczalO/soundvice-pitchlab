import { useEffect, useState } from "react";
import { DataPanel } from "../components/DataPanel";
import { modelLabel, useModels } from "../components/Models";
import { api, toAppError, type SecretName } from "../lib/api";
import { useApp, type Theme } from "../lib/store";

function KeyField(props: {
  name: SecretName;
  label: string;
  placeholder: string;
  saved: boolean;
  hint?: string;
}) {
  const { refreshKeys, notify } = useApp();
  const [value, setValue] = useState("");
  const id = `key-${props.name}`;

  const save = async () => {
    try {
      await api.setSecret(props.name, value);
      setValue("");
      await refreshKeys();
      notify("Klucz zapisany w sejfie systemowym");
    } catch (e) {
      notify(toAppError(e).message);
    }
  };
  const remove = async () => {
    try {
      await api.deleteSecret(props.name);
      await refreshKeys();
      notify("Klucz usunięty");
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  return (
    <div className="field">
      <label htmlFor={id}>{props.label}</label>
      <div className="row">
        <input
          id={id}
          type="password"
          autoComplete="off"
          style={{ flex: "1 1 150px" }}
          placeholder={props.saved ? "•••••••• zapisany w sejfie" : props.placeholder}
          value={value}
          onChange={(e) => setValue(e.target.value)}
        />
        <button className="btn" disabled={!value.trim()} onClick={save}>
          {props.saved ? "Zmień" : "Zapisz"}
        </button>
        {props.saved && (
          <button className="btn ghost danger" onClick={remove}>
            Usuń
          </button>
        )}
      </div>
      {props.hint && <span className="hint">{props.hint}</span>}
    </div>
  );
}

export function Settings() {
  const { settings, saveSetting, hasGemini, hasAnthropic, notify } = useApp();
  const { models, status, load, loading, error } = useModels();
  const [version, setVersion] = useState("");
  const [testing, setTesting] = useState(false);
  const [claudeModel, setClaudeModel] = useState(settings.claude_model ?? "claude-opus-5-5");
  const engine = settings.engine ?? "gemini";

  useEffect(() => {
    api
      .appVersion()
      .then(setVersion)
      .catch(() => {});
  }, []);

  const options = (current?: string) => {
    const list = models.length ? models : current ? [current] : [];
    return list.map((m) => (
      <option key={m} value={m}>
        {modelLabel(m, status)}
      </option>
    ));
  };

  const test = async () => {
    setTesting(true);
    try {
      const r = await api.testConnection({
        geminiModel: settings.gemini_model,
        claudeModel,
        checkClaude: engine === "claude",
      });
      notify(`Gemini: ${r.gemini}${r.claude ? ` · Claude: ${r.claude}` : ""}`);
    } catch (e) {
      notify(toAppError(e).message);
    } finally {
      setTesting(false);
    }
  };

  return (
    <section className="view">
      <div className="view-head">
        <div className="eyebrow">Połączenia</div>
        <h1>Ustawienia</h1>
        <p className="lede">
          Klucze są przechowywane w sejfie systemowym (Menedżer poświadczeń Windows / Pęk kluczy
          macOS). Aplikacja łączy się bezpośrednio z API Google i Anthropic — bez serwera
          pośredniego.
        </p>
      </div>
      <div className="grid-2">
        <div className="panel">
          <h2>Gemini — odsłuch i research</h2>
          <KeyField
            name="gemini"
            label="Klucz API Gemini"
            placeholder="AIza…"
            saved={hasGemini}
            hint="Z Google AI Studio → Get API key."
          />
          <div className="row">
            <button className="btn" disabled={!hasGemini || loading} onClick={() => load(false)}>
              {loading ? "Sprawdzam modele…" : "Pobierz i sprawdź modele"}
            </button>
            <span className="hint">
              {error ??
                (models.length
                  ? `${models.filter((m) => status[m] === "ok").length} z ${models.length} modeli działa dla Twojego klucza`
                  : "")}
            </span>
          </div>
          <div className="field">
            <label htmlFor="gModel">Model do odsłuchu i analizy</label>
            <select
              id="gModel"
              value={settings.gemini_model ?? ""}
              onChange={(e) => saveSetting("gemini_model", e.target.value)}
            >
              {options(settings.gemini_model)}
            </select>
          </div>
          <div className="field">
            <label htmlFor="gModelR">Model do researchu (Google Search)</label>
            <select
              id="gModelR"
              value={settings.gemini_model_research ?? ""}
              onChange={(e) => saveSetting("gemini_model_research", e.target.value)}
            >
              {options(settings.gemini_model_research)}
            </select>
          </div>
          <div className="sep" />
          <h2>Silnik pitchu</h2>
          <div className="field">
            <label htmlFor="engine">Kto pisze pitch</label>
            <select
              id="engine"
              value={engine}
              onChange={(e) => saveSetting("engine", e.target.value)}
            >
              <option value="gemini">Gemini (ten sam klucz)</option>
              <option value="claude">Claude (klucz Anthropic API)</option>
            </select>
          </div>
          {engine === "claude" && (
            <>
              <KeyField
                name="anthropic"
                label="Klucz Anthropic API"
                placeholder="sk-ant-…"
                saved={hasAnthropic}
              />
              <div className="field">
                <label htmlFor="cModel">Model Claude</label>
                <input
                  id="cModel"
                  type="text"
                  value={claudeModel}
                  onChange={(e) => setClaudeModel(e.target.value)}
                  onBlur={() =>
                    saveSetting("claude_model", claudeModel.trim() || "claude-opus-5-5")
                  }
                />
                <span className="hint">Np. claude-opus-5-5 albo tańszy claude-sonnet-5-5.</span>
              </div>
            </>
          )}
          <div className="row">
            <button className="btn primary" disabled={!hasGemini || testing} onClick={test}>
              {testing ? "Sprawdzam…" : "Sprawdź połączenie"}
            </button>
          </div>
        </div>
        <div className="panel">
          <h2>Wygląd</h2>
          <div className="field">
            <label htmlFor="theme">Motyw</label>
            <select
              id="theme"
              value={settings.theme ?? "system"}
              onChange={(e) => saveSetting("theme", e.target.value as Theme)}
            >
              <option value="system">Systemowy</option>
              <option value="light">Jasny</option>
              <option value="dark">Ciemny</option>
            </select>
          </div>
          <div className="sep" />
          <DataPanel />
          <div className="sep" />
          <h2>O aplikacji</h2>
          <p className="hint">Soundvice pitchLAB · wersja {version || "—"}</p>
        </div>
      </div>
    </section>
  );
}
