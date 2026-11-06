import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { Search, Square } from "lucide-react";
import { Steps, type StepMap } from "../components/Steps";
import {
  api,
  toAppError,
  type ArtistDetail,
  type ArtistRow,
  type DoneEvent,
  type HistoryRow,
  type StepEvent,
} from "../lib/api";
import { sideBySide, type Seg } from "../lib/diff";
import { fmtDateTime } from "../lib/format";
import { useApp } from "../lib/store";

const CHANGE_LABEL: Record<HistoryRow["change_type"], string> = {
  manual: "ręczna edycja",
  research_accept: "przyjęty research",
  append: "dopisany research",
  restore: "przywrócenie",
};

function Highlighted({ segs, cls }: { segs: Seg[]; cls: string }) {
  return (
    <pre className="raw">
      {segs.map((s, i) =>
        s.changed ? (
          <mark key={i} className={cls}>
            {s.text}
          </mark>
        ) : (
          s.text
        ),
      )}
    </pre>
  );
}

function Avatar({ name }: { name: string }) {
  return (
    <span className="avatar" aria-hidden="true">
      {name.trim().charAt(0).toUpperCase()}
    </span>
  );
}

interface EditorProps {
  detail: ArtistDetail;
  reload: () => void;
  onDeleted: () => void;
}

function Editor({ detail: a, reload, onDeleted }: EditorProps) {
  const { notify, openTrack } = useApp();
  const [pill, setPill] = useState(a.pill);
  const [notes, setNotes] = useState(a.manager_notes);
  const [hints, setHints] = useState(a.research_hints);
  const [confirmDel, setConfirmDel] = useState(false);
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<number | null>(null);
  const [steps, setSteps] = useState<StepMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const runningRef = useRef(false);

  useEffect(() => {
    const subs = [
      listen<StepEvent>("job-step", (e) => {
        if (!runningRef.current) return;
        const { step, state, sub } = e.payload;
        setSteps((s) => ({ ...(s ?? {}), [step]: { state, sub: sub ?? s?.[step]?.sub ?? "" } }));
      }),
      listen<DoneEvent>("job-done", (e) => {
        if (!runningRef.current) return;
        runningRef.current = false;
        setRunning(false);
        setJobId(null);
        if (e.payload.cancelled) notify("Zatrzymano");
        else if (e.payload.error) setError(e.payload.error.message);
        reload();
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, [notify, reload]);

  const save = async () => {
    try {
      await api.updateArtist({
        id: a.id,
        pill,
        managerNotes: notes,
        researchHints: hints,
      });
      notify("Zapisano");
      reload();
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  const research = async () => {
    setError(null);
    setSteps({});
    try {
      // Wskazówki i notatki wpływają na research, więc najpierw zapisujemy bieżące pola.
      await api.updateArtist({ id: a.id, pill, managerNotes: notes, researchHints: hints });
      runningRef.current = true;
      setRunning(true);
      setJobId(await api.startResearch(a.id));
    } catch (e) {
      runningRef.current = false;
      setRunning(false);
      setError(toAppError(e).message);
    }
  };

  const act = async (fn: () => Promise<void>, ok: string) => {
    try {
      await fn();
      notify(ok);
      reload();
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  const diff = a.proposal ? sideBySide(a.pill, a.proposal.pill) : null;

  return (
    <>
      <div className="panel-h">
        <div className="sec-title">
          <Avatar name={a.name} />
          <div>
            <div className="eyebrow">Artysta</div>
            <h2>{a.name}</h2>
          </div>
        </div>
        <div className="row">
          <button className="btn" disabled={running} onClick={research}>
            <Search size={17} aria-hidden="true" />
            {a.pill.trim() ? "Odśwież research" : "Zrób research"}
          </button>
          {running && (
            <button className="btn ghost" onClick={() => jobId != null && api.cancelJob(jobId)}>
              <Square size={17} aria-hidden="true" />
              Zatrzymaj
            </button>
          )}
          {!confirmDel ? (
            <button className="btn ghost danger" onClick={() => setConfirmDel(true)}>
              Usuń
            </button>
          ) : (
            <button
              className="btn danger"
              onClick={() => act(() => api.deleteArtist(a.id).then(onDeleted), "Usunięto")}
            >
              Potwierdź usunięcie
            </button>
          )}
        </div>
      </div>

      {steps && <Steps steps={steps} only={["research", "pill"]} />}
      {error && <div className="err-box">{error}</div>}

      {a.proposal && diff && (
        <div className="proposal">
          <div className="row" style={{ justifyContent: "space-between" }}>
            <h3>Propozycja aktualizacji z researchu · {fmtDateTime(a.proposal.created_at)}</h3>
            <div className="row">
              <button
                className="btn primary"
                onClick={() =>
                  act(() => api.acceptProposal(a.proposal!.id, "replace"), "Pigułka zaktualizowana")
                }
              >
                Zastąp pigułkę
              </button>
              <button
                className="btn"
                onClick={() =>
                  act(
                    () => api.acceptProposal(a.proposal!.id, "append"),
                    "Dopisano — przytnij ręcznie",
                  )
                }
              >
                Dopisz na końcu
              </button>
              <button
                className="btn ghost"
                onClick={() => act(() => api.rejectProposal(a.proposal!.id), "Odrzucono")}
              >
                Odrzuć
              </button>
            </div>
          </div>
          <p className="hint">
            Twoja pigułka pozostaje bez zmian, dopóki nie przyjmiesz propozycji. Zmienione fragmenty
            są podświetlone.
          </p>
          <div className="cmp">
            <div>
              <div className="lbl">Twoja pigułka</div>
              <Highlighted segs={diff.left} cls="diff-del" />
            </div>
            <div>
              <div className="lbl">Propozycja</div>
              <Highlighted segs={diff.right} cls="diff-add" />
            </div>
          </div>
        </div>
      )}

      <div className="field">
        <label htmlFor="aPill">
          Pigułka (edytuj dowolnie — tak będzie widziana w każdym pitchu)
        </label>
        <textarea
          id="aPill"
          className="pill"
          placeholder={
            "KIM JEST:\nBRZMIENIE I TEMATY:\nKAMIENIE MILOWE:\nWYDAWNICTWA:\nWSPÓŁPRACE:\nNARRACJA DO PITCHU:\nNIEPEWNE / NIE UŻYWAĆ:"
          }
          value={pill}
          onChange={(e) => setPill(e.target.value)}
        />
        <span className="hint">Oznacz fakt [?], żeby generator go pominął.</span>
      </div>
      <div className="field">
        <label htmlFor="aNotes">Notatki managera — najwyższy priorytet</label>
        <textarea
          id="aNotes"
          placeholder="Co ZAWSZE / NIGDY nie mówić, preferowane ujęcie, aktualne liczby z S4A, tajne jeszcze informacje, ton…"
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />
      </div>
      <div className="field">
        <label htmlFor="aHints">Wskazówki do researchu</label>
        <input
          id="aHints"
          type="text"
          placeholder="np. raper z Opola, nie mylić z DJ-em o tym samym pseudonimie; IG @…"
          value={hints}
          onChange={(e) => setHints(e.target.value)}
        />
      </div>
      <div className="row">
        <button className="btn primary" onClick={save}>
          Zapisz
        </button>
        <span className="hint">
          Zapisano: {fmtDateTime(a.updated_at)} · research: {fmtDateTime(a.researched_at)}
        </span>
      </div>

      {a.sources.length > 0 && (
        <details>
          <summary>Źródła researchu ({a.sources.length})</summary>
          <ol className="src">
            {a.sources.map((s) => (
              <li key={s.url}>
                <a href={s.url} target="_blank" rel="noopener noreferrer">
                  {s.title}
                </a>
              </li>
            ))}
          </ol>
          <p className="hint">Linki Google mogą prowadzić przez przekierowanie vertexaisearch.</p>
        </details>
      )}
      {a.dossier && (
        <details>
          <summary>Pełne dossier z researchu</summary>
          <pre className="raw">{a.dossier}</pre>
        </details>
      )}
      {a.history.length > 0 && (
        <details>
          <summary>Historia pigułki ({a.history.length})</summary>
          <div style={{ display: "grid", gap: 8, marginTop: 10 }}>
            {a.history.map((h) => (
              <div className="hist-item" key={h.id}>
                <div className="m">
                  {fmtDateTime(h.changed_at)} · zastąpiona przez: {CHANGE_LABEL[h.change_type]}
                </div>
                <div className="pv">{h.pill}</div>
                <div>
                  <button
                    className="btn ghost"
                    onClick={() =>
                      act(
                        () => api.restorePill(h.id),
                        "Przywrócono — obecna wersja trafiła do historii",
                      )
                    }
                  >
                    Przywróć tę wersję
                  </button>
                </div>
              </div>
            ))}
          </div>
        </details>
      )}
      {a.tracks.length > 0 && (
        <details>
          <summary>Pitche tego artysty ({a.tracks.length})</summary>
          <div style={{ display: "grid", gap: 6, marginTop: 8 }}>
            {a.tracks.map((t) => (
              <button
                key={t.id}
                className="btn ghost"
                style={{ justifySelf: "start" }}
                onClick={() => openTrack(t.id)}
              >
                {t.title || "Bez tytułu"} · {fmtDateTime(t.created_at)}
              </button>
            ))}
          </div>
        </details>
      )}
    </>
  );
}

export function Artists({ active }: { active: boolean }) {
  const notify = useApp((s) => s.notify);
  const [list, setList] = useState<ArtistRow[]>([]);
  const [selected, setSelected] = useState<number | null>(null);
  const [detail, setDetail] = useState<ArtistDetail | null>(null);
  const [query, setQuery] = useState("");
  const [newName, setNewName] = useState("");
  const [rev, setRev] = useState(0);
  const reload = useCallback(() => setRev((r) => r + 1), []);

  useEffect(() => {
    if (!active) return;
    let alive = true;
    api
      .listArtists()
      .then((l) => alive && setList(l))
      .catch((e) => notify(toAppError(e).message));
    return () => {
      alive = false;
    };
  }, [active, rev, notify]);

  useEffect(() => {
    if (!active || selected == null) return;
    let alive = true;
    api
      .getArtist(selected)
      .then((d) => alive && setDetail(d))
      .catch((e) => notify(toAppError(e).message));
    return () => {
      alive = false;
    };
  }, [active, selected, rev, notify]);

  const add = async () => {
    if (!newName.trim()) return;
    try {
      const id = await api.createArtist(newName);
      setNewName("");
      setSelected(id);
      reload();
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  const q = query.trim().toLowerCase();
  const shown = list.filter((a) => !q || a.name.toLowerCase().includes(q));
  const current = detail && detail.id === selected ? detail : null;

  return (
    <section className="view">
      <div className="view-head">
        <div className="eyebrow">Pamięć</div>
        <h1>Pigułki artystów</h1>
        <p className="lede">
          Pigułka to skrót wiedzy o artyście używany w każdym pitchu. Research jej nie nadpisuje —
          nowe ustalenia przychodzą jako propozycja do przejęcia. Notatki managera mają najwyższy
          priorytet i nigdy nie są zmieniane automatycznie.
        </p>
      </div>
      <div className="grid-art">
        <div className="panel">
          <div className="row">
            <input
              type="text"
              placeholder="Dodaj artystę"
              aria-label="Nazwa nowego artysty"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && add()}
            />
            <button className="btn" onClick={add}>
              Dodaj
            </button>
          </div>
          <input
            className="search"
            type="text"
            placeholder="Szukaj artysty"
            aria-label="Szukaj artysty"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
          <div className="art-list">
            {shown.length ? (
              shown.map((a) => (
                <button
                  key={a.id}
                  aria-current={a.id === selected}
                  onClick={() => setSelected(a.id)}
                >
                  <Avatar name={a.name} />
                  <span style={{ minWidth: 0, display: "grid" }}>
                    <span className="n">
                      <span>{a.name}</span>
                      {a.has_proposal && <span className="badge">nowe</span>}
                    </span>
                    <span className="m">
                      {a.has_pill ? `pigułka · ${fmtDateTime(a.updated_at)}` : "brak pigułki"}
                    </span>
                  </span>
                </button>
              ))
            ) : (
              <p className="empty">{list.length ? "Brak wyników." : "Brak artystów."}</p>
            )}
          </div>
        </div>
        <div className="panel">
          {current ? (
            <Editor
              key={`${current.id}-${current.updated_at}`}
              detail={current}
              reload={reload}
              onDeleted={() => {
                setSelected(null);
                setDetail(null);
                reload();
              }}
            />
          ) : (
            <p className="empty">
              Wybierz artystę z listy albo dodaj nowego. Artyści dopisują się też sami po pierwszej
              analizie utworu.
            </p>
          )}
        </div>
      </div>
    </section>
  );
}
