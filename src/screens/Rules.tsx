import { useCallback, useEffect, useState } from "react";
import { api, toAppError, type RuleVersion, type RulesState } from "../lib/api";
import { fmtDateTime } from "../lib/format";
import { useApp } from "../lib/store";

function Versions({ items, onRestore }: { items: RuleVersion[]; onRestore: (id: number) => void }) {
  if (items.length <= 1) return null;
  return (
    <details>
      <summary>Historia wersji ({items.length})</summary>
      <div style={{ display: "grid", gap: 8, marginTop: 10 }}>
        {items.map((v, i) => (
          <div className="hist-item" key={v.id}>
            <div className="m">
              wersja {v.version} · {fmtDateTime(v.created_at)}
              {i === 0 ? " · aktualna" : ""}
            </div>
            <div className="pv">{v.body || "(pusto)"}</div>
            {i > 0 && (
              <div>
                <button className="btn ghost" onClick={() => onRestore(v.id)}>
                  Przywróć tę wersję
                </button>
              </div>
            )}
          </div>
        ))}
      </div>
    </details>
  );
}

function RulesForm({ state, reload }: { state: RulesState; reload: () => void }) {
  const { notify, saveSetting } = useApp();
  const [rules, setRules] = useState(state.rules);
  const [examples, setExamples] = useState(state.examples);
  const [lang, setLang] = useState(state.lang);

  const run = async (fn: () => Promise<unknown>, ok: string) => {
    try {
      await fn();
      notify(ok);
      reload();
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  return (
    <div className="grid-2">
      <div className="panel">
        <div className="field">
          <label htmlFor="rulesTxt">Zasady</label>
          <textarea
            id="rulesTxt"
            className="mono"
            style={{ minHeight: 460 }}
            value={rules}
            onChange={(e) => setRules(e.target.value)}
          />
        </div>
        <div className="row">
          <button
            className="btn primary"
            disabled={rules === state.rules}
            onClick={() =>
              run(async () => {
                const changed = await api.saveRules("rules", rules);
                if (!changed) throw new Error("Bez zmian");
              }, "Zapisano zasady (nowa wersja)")
            }
          >
            Zapisz zasady
          </button>
          <button
            className="btn ghost"
            disabled={state.rules.trim() === state.default_rules.trim()}
            onClick={() =>
              run(
                () => api.resetDefaultRules(),
                "Przywrócono domyślne zasady (poprzednie są w historii)",
              )
            }
          >
            Przywróć domyślne
          </button>
        </div>
        <Versions
          items={state.rules_versions}
          onRestore={(id) => run(() => api.restoreRuleVersion(id), "Przywrócono wersję zasad")}
        />
      </div>
      <div className="panel">
        <div className="field">
          <label htmlFor="examplesTxt">Wzorcowe pitche</label>
          <textarea
            id="examplesTxt"
            style={{ minHeight: 300 }}
            placeholder="Wklej 1–5 pitchy, które przeszły albo które uważasz za wzorcowe. Oddziel je linią ---"
            value={examples}
            onChange={(e) => setExamples(e.target.value)}
          />
          <span className="hint">Przycisk „Dodaj do wzorców” przy pitchu dopisuje go tutaj.</span>
        </div>
        <div className="field">
          <label htmlFor="pitchLang">Język pitchu</label>
          <select
            id="pitchLang"
            value={lang}
            onChange={(e) => {
              setLang(e.target.value);
              void saveSetting("pitch_lang", e.target.value);
            }}
          >
            <option value="en">angielski (zalecany dla redakcji Spotify)</option>
            <option value="pl">polski</option>
          </select>
        </div>
        <div className="row">
          <button
            className="btn primary"
            disabled={examples === state.examples}
            onClick={() =>
              run(async () => {
                await api.saveRules("examples", examples);
              }, "Zapisano wzorce (nowa wersja)")
            }
          >
            Zapisz wzorce
          </button>
        </div>
        <Versions
          items={state.examples_versions}
          onRestore={(id) => run(() => api.restoreRuleVersion(id), "Przywrócono wersję wzorców")}
        />
      </div>
    </div>
  );
}

export function Rules() {
  const notify = useApp((s) => s.notify);
  const [state, setState] = useState<RulesState | null>(null);
  const [rev, setRev] = useState(0);
  const reload = useCallback(() => setRev((r) => r + 1), []);

  useEffect(() => {
    let alive = true;
    api
      .getRules()
      .then((s) => alive && setState(s))
      .catch((e) => notify(toAppError(e).message));
    return () => {
      alive = false;
    };
  }, [rev, notify]);

  return (
    <section className="view">
      <div className="view-head">
        <div className="eyebrow">Instrukcje dla generatora</div>
        <h1>Zasady pitchu</h1>
        <p className="lede">
          To, co tu napiszesz, trafia dosłownie do modelu przy każdym pitchu. Wzorcowe pitche
          (zatwierdzone przez Ciebie) działają jak przykłady stylu. Każdy zapis tworzy nową wersję —
          poprzednie możesz przywrócić.
        </p>
      </div>
      {state && (
        <RulesForm
          key={`${state.rules_versions[0]?.id}-${state.examples_versions[0]?.id}`}
          state={state}
          reload={reload}
        />
      )}
    </section>
  );
}
