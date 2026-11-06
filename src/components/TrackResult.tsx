import { useState } from "react";
import { Copy, Sparkles } from "lucide-react";
import { api, toAppError, type PitchView, type TrackView } from "../lib/api";
import { fmtDateTime } from "../lib/format";
import { PITCH_LIMIT, countChars, meterSegments } from "../lib/meter";
import { useApp } from "../lib/store";
import { AnalysisView } from "./AnalysisView";

const Chips = ({ items, accent }: { items?: string[]; accent?: boolean }) =>
  items?.length ? (
    <div className={`chips${accent ? " acc" : ""}`}>
      {items.map((x) => (
        <span className="chip" key={x}>
          {x}
        </span>
      ))}
    </div>
  ) : (
    <span className="hint">—</span>
  );

function PitchEditor({
  pitch,
  trackId,
  busy,
  onChanged,
  onJob,
}: {
  pitch: PitchView;
  trackId: number;
  busy: boolean;
  onChanged: () => void;
  onJob: (mode: "new" | "revise", feedback?: string, currentText?: string) => void;
}) {
  const notify = useApp((s) => s.notify);
  const [text, setText] = useState(pitch.text);
  const [feedback, setFeedback] = useState("");
  const n = countChars(text);
  const over = n > PITCH_LIMIT;

  const guard = async (fn: () => Promise<void>, ok: string) => {
    try {
      await fn();
      notify(ok);
    } catch (e) {
      notify(toAppError(e).message);
    }
  };

  return (
    <>
      <div className="pitch-box field">
        <label htmlFor="pitchTxt">Pitch</label>
        <textarea id="pitchTxt" value={text} onChange={(e) => setText(e.target.value)} />
        <div className="meter" aria-hidden="true">
          {meterSegments(n).map((s, i) => (
            <i key={i} className={s.on ? `on${s.hot ? " hot" : ""}${s.over ? " over" : ""}` : ""} />
          ))}
        </div>
        <div className="meter-scale" aria-hidden="true">
          {[0, 100, 200, 300, 400, 500].map((v) => (
            <span key={v}>{v}</span>
          ))}
        </div>
        <div className="row" style={{ justifyContent: "space-between" }}>
          <span className={`count${over ? " over" : ""}`} role="status">
            {n} / {PITCH_LIMIT} znaków{over ? " — za długo dla Spotify" : ""}
          </span>
          <span className="hint">
            {pitch.engine === "claude" ? "Claude" : "Gemini"} · {pitch.model} ·{" "}
            {fmtDateTime(pitch.created_at)}
            {pitch.is_manual_edit ? " · edycja ręczna" : ""}
          </span>
        </div>
      </div>
      <div className="pitch-actions">
        <button
          className="btn primary"
          onClick={() =>
            navigator.clipboard
              .writeText(text)
              .then(() => notify("Skopiowano"))
              .catch(() => notify("Nie udało się skopiować — zaznacz tekst ręcznie"))
          }
        >
          <Copy size={17} aria-hidden="true" />
          Kopiuj pitch
        </button>
        <button
          className="btn"
          disabled={busy || text.trim() === pitch.text}
          onClick={() =>
            guard(async () => {
              await api.savePitchEdit(trackId, text);
              onChanged();
            }, "Zapisano edycję")
          }
        >
          Zapisz moją edycję
        </button>
        <button className="btn" disabled={busy} onClick={() => onJob("new")}>
          <Sparkles size={17} aria-hidden="true" />
          Nowa wersja
        </button>
        <button
          className="btn ghost"
          onClick={() => guard(() => api.addToExamples(text), "Dodano do wzorców")}
        >
          Dodaj do wzorców
        </button>
      </div>
      <div className="field">
        <label htmlFor="feedback">Popraw wg uwag</label>
        <div className="row">
          <input
            id="feedback"
            type="text"
            style={{ flex: "1 1 240px" }}
            placeholder="np. mocniej o teledysku, bez liczb, krócej pierwsze zdanie"
            value={feedback}
            onChange={(e) => setFeedback(e.target.value)}
            onKeyDown={(e) =>
              e.key === "Enter" && feedback.trim() && !busy && onJob("revise", feedback, text)
            }
          />
          <button
            className="btn"
            disabled={busy || !feedback.trim()}
            onClick={() => onJob("revise", feedback, text)}
          >
            Popraw
          </button>
        </div>
      </div>
    </>
  );
}

/** Wynik dla utworu: pitch (edycja, wersje, poprawki), pola S4A, fakty, analiza, kontekst. */
export function TrackResult({
  track,
  busy,
  onChanged,
  onJob,
}: {
  track: TrackView;
  busy: boolean;
  onChanged: () => void;
  onJob: (mode: "fresh" | "new" | "revise", feedback?: string, currentText?: string) => void;
}) {
  const { notify, openArtist } = useApp();
  const current = track.pitches.find((p) => p.is_current);
  const versions = track.pitches.filter((p) => !p.is_current);
  const form = current?.form ?? {};

  return (
    <>
      <div className="result-head">
        <div>
          <div className="eyebrow">
            {track.artist_name}
            {track.release_date ? ` · premiera ${track.release_date}` : ""}
          </div>
          <h2 style={{ marginTop: 6 }}>{track.title || "Bez tytułu"}</h2>
        </div>
      </div>

      {current ? (
        <PitchEditor
          key={current.id}
          pitch={current}
          trackId={track.id}
          busy={busy}
          onChanged={onChanged}
          onJob={onJob}
        />
      ) : (
        <>
          <p className="hint">Analiza jest gotowa, ale pitch się nie wygenerował.</p>
          <div className="row">
            <button className="btn primary" disabled={busy} onClick={() => onJob("fresh")}>
              <Sparkles size={17} aria-hidden="true" />
              Wygeneruj pitch
            </button>
          </div>
        </>
      )}

      {current && (
        <>
          <details open>
            <summary>Tłumaczenie robocze (PL)</summary>
            <p>{current.text_pl || "—"}</p>
            {current.angle && <p className="hint">Kąt: {current.angle}</p>}
          </details>
          <details open>
            <summary>Pola Spotify for Artists</summary>
            <dl className="kv" style={{ marginTop: 10 }}>
              <dt>Gatunki</dt>
              <dd>
                <Chips items={form.genres} accent />
              </dd>
              <dt>Nastroje</dt>
              <dd>
                <Chips items={form.moods} accent />
              </dd>
              <dt>Style</dt>
              <dd>
                <Chips items={form.styles} />
              </dd>
              <dt>Instrumenty</dt>
              <dd>
                <Chips items={form.instruments} />
              </dd>
              <dt>Kultura</dt>
              <dd>{form.culture || "—"}</dd>
              <dt>Typ utworu</dt>
              <dd>{form.song_type || "—"}</dd>
              <dt>Język wokalu</dt>
              <dd>{form.vocal_language || "—"}</dd>
            </dl>
          </details>
          {current.facts_used.length > 0 && (
            <details>
              <summary>Użyte fakty (do weryfikacji)</summary>
              <ul>
                {current.facts_used.map((f, i) => (
                  <li key={i}>{f}</li>
                ))}
              </ul>
            </details>
          )}
        </>
      )}

      {track.analysis && (
        <details>
          <summary>Analiza odsłuchu</summary>
          <AnalysisView an={track.analysis} />
        </details>
      )}

      {versions.length > 0 && (
        <details>
          <summary>Poprzednie wersje ({versions.length})</summary>
          <div style={{ display: "grid", gap: 8, marginTop: 10 }}>
            {versions.map((v) => (
              <div className="ver" key={v.id}>
                <div className="m">
                  {fmtDateTime(v.created_at)} · {countChars(v.text)} znaków
                  {v.is_manual_edit ? " · edycja ręczna" : ""}
                </div>
                <div>{v.text}</div>
                <div>
                  <button
                    className="btn ghost"
                    disabled={busy}
                    onClick={async () => {
                      try {
                        await api.restorePitch(v.id);
                        onChanged();
                        notify("Przywrócono");
                      } catch (e) {
                        notify(toAppError(e).message);
                      }
                    }}
                  >
                    Przywróć
                  </button>
                </div>
              </div>
            ))}
          </div>
        </details>
      )}

      <details>
        <summary>Kontekst użyty do pitchu</summary>
        <p className="hint">{track.campaign_context || "brak kontekstu kampanii"}</p>
        <p>
          <button className="btn ghost" onClick={() => openArtist(track.artist_id)}>
            Otwórz pigułkę: {track.artist_name}
          </button>
        </p>
      </details>
    </>
  );
}
