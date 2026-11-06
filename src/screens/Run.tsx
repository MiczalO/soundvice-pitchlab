import { useCallback, useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import { AudioLines, Square, Upload, Sparkles } from "lucide-react";
import { AnalysisView } from "../components/AnalysisView";
import { Player } from "../components/Player";
import { Steps, type StepMap } from "../components/Steps";
import {
  api,
  toAppError,
  type ArtistRow,
  type DoneEvent,
  type PreparedAudio,
  type StepEvent,
  type TrackView,
} from "../lib/api";
import { fmtSize, titleFromFile } from "../lib/format";
import { useApp } from "../lib/store";

const AUDIO_EXT = ["mp3", "wav", "flac", "m4a", "aac", "ogg", "aiff", "aif"];

export function Run({ active }: { active: boolean }) {
  const notify = useApp((s) => s.notify);
  const openTrackId = useApp((s) => s.openTrackId);
  const [artistNames, setArtistNames] = useState<ArtistRow[]>([]);
  const [forceResearch, setForceResearch] = useState(false);
  const [finished, setFinished] = useState(false);
  const [audio, setAudio] = useState<PreparedAudio | null>(null);
  const [preparing, setPreparing] = useState(false);
  const [dragOver, setDragOver] = useState(false);
  const [title, setTitle] = useState("");
  const [artist, setArtist] = useState("");
  const [featuring, setFeaturing] = useState("");
  const [date, setDate] = useState("");
  const [ctx, setCtx] = useState("");
  const [lyrics, setLyrics] = useState("");
  const [running, setRunning] = useState(false);
  const [jobId, setJobId] = useState<number | null>(null);
  const [steps, setSteps] = useState<StepMap | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [track, setTrack] = useState<TrackView | null>(null);
  const runningRef = useRef(false);

  const loadFile = useCallback(
    async (path: string) => {
      const ext = path.split(".").pop()?.toLowerCase() ?? "";
      if (!AUDIO_EXT.includes(ext)) {
        notify("Wybierz plik MP3, WAV, FLAC, M4A, OGG lub AIFF");
        return;
      }
      setPreparing(true);
      setError(null);
      try {
        const a = await api.prepareAudio(path);
        setAudio(a);
        setTitle((t) => t || titleFromFile(a.file_name));
      } catch (e) {
        notify(toAppError(e).message);
      } finally {
        setPreparing(false);
      }
    },
    [notify],
  );

  const pick = useCallback(async () => {
    const sel = await open({
      multiple: false,
      filters: [{ name: "Audio", extensions: AUDIO_EXT }],
    });
    if (typeof sel === "string") await loadFile(sel);
  }, [loadFile]);

  // Upuszczanie plików (Tauri podaje ścieżki, nie obiekty File).
  useEffect(() => {
    if (!active) return;
    let un: (() => void) | undefined;
    let cancelled = false;
    getCurrentWebview()
      .onDragDropEvent((e) => {
        const p = e.payload;
        if (p.type === "enter" || p.type === "over") setDragOver(true);
        else if (p.type === "leave") setDragOver(false);
        else if (p.type === "drop") {
          setDragOver(false);
          if (p.paths[0]) void loadFile(p.paths[0]);
        }
      })
      .then((f) => (cancelled ? f() : (un = f)));
    return () => {
      cancelled = true;
      un?.();
    };
  }, [active, loadFile]);

  useEffect(() => {
    api
      .listArtists()
      .then(setArtistNames)
      .catch(() => {});
  }, []);

  useEffect(() => {
    if (openTrackId == null) return;
    let alive = true;
    api.getTrack(openTrackId).then((t) => alive && t && setTrack(t));
    return () => {
      alive = false;
    };
  }, [openTrackId]);

  // Postęp i zakończenie joba.
  useEffect(() => {
    const subs = [
      listen<StepEvent>("job-step", (e) => {
        if (!runningRef.current) return;
        const { step, state, sub } = e.payload;
        setSteps((s) => ({ ...(s ?? {}), [step]: { state, sub: sub ?? s?.[step]?.sub ?? "" } }));
      }),
      listen<DoneEvent>("job-done", async (e) => {
        if (!runningRef.current) return;
        runningRef.current = false;
        setRunning(false);
        setJobId(null);
        const d = e.payload;
        if (d.cancelled) notify("Zatrzymano");
        else if (d.error) setError(d.error.message);
        else setFinished(true);
        if (d.track_id != null) setTrack(await api.getTrack(d.track_id));
        api
          .listArtists()
          .then(setArtistNames)
          .catch(() => {});
      }),
    ];
    return () => {
      subs.forEach((p) => p.then((f) => f()));
    };
  }, [notify]);

  const start = async () => {
    if (!audio) return notify("Najpierw wybierz plik audio");
    if (!artist.trim()) return notify("Wpisz artystę");
    setError(null);
    setFinished(false);
    setTrack(null);
    setSteps({});
    runningRef.current = true;
    setRunning(true);
    try {
      const id = await api.startAnalysis({
        filePath: audio.path,
        hash: audio.hash,
        mime: audio.mime,
        fileName: audio.file_name,
        size: audio.size,
        durationS: audio.waveform?.duration_s ?? null,
        title: title.trim(),
        artist: artist.trim(),
        featuring: featuring.trim(),
        releaseDate: date || null,
        campaignContext: ctx,
        lyrics,
        forceResearch,
      });
      setJobId(id);
    } catch (e) {
      runningRef.current = false;
      setRunning(false);
      setError(toAppError(e).message);
    }
  };

  const stop = () => {
    if (jobId != null) void api.cancelJob(jobId);
  };

  return (
    <section className="view">
      <div className="view-head">
        <div className="eyebrow">
          <span className="flow">
            Utwór <i>→</i> odsłuch <i>→</i> research <i>→</i> pitch
          </span>
        </div>
        <h1>Nowy pitch do Spotify</h1>
        <p className="lede">
          Wgraj master, podaj artystę i kontekst kampanii. Gemini odsłucha utwór, a jeśli artysta
          nie ma jeszcze pigułki, zrobi research i ją założy. Pitch powstaje z pigułki, Twoich
          notatek i analizy.
        </p>
      </div>
      <div className="grid-2">
        <div className="panel">
          {audio ? (
            <div className="drop has">
              <Player
                key={audio.path}
                path={audio.path}
                name={audio.file_name}
                meta={fmtSize(audio.size)}
                peaks={audio.waveform?.peaks ?? null}
                onSwap={pick}
              />
            </div>
          ) : (
            <div
              className={`drop${dragOver ? " on" : ""}`}
              tabIndex={0}
              role="button"
              aria-label="Wybierz plik audio"
              onClick={pick}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  void pick();
                }
              }}
            >
              <span className="ico">
                <Upload size={20} aria-hidden="true" />
              </span>
              <strong>{preparing ? "Przygotowuję plik…" : "Upuść master albo kliknij"}</strong>
              <span className="hint">
                MP3, WAV, FLAC, M4A, OGG, AIFF · duże pliki idą przez Files API
              </span>
            </div>
          )}
          <div className="row">
            <div className="field">
              <label htmlFor="fTitle">Tytuł utworu</label>
              <input
                id="fTitle"
                type="text"
                placeholder="np. RETRO"
                value={title}
                onChange={(e) => setTitle(e.target.value)}
              />
            </div>
            <div className="field">
              <label htmlFor="fArtist">Artysta</label>
              <input
                id="fArtist"
                type="text"
                placeholder="Pseudonim"
                list="artistOpts"
                value={artist}
                onChange={(e) => setArtist(e.target.value)}
              />
              <datalist id="artistOpts">
                {artistNames.map((a) => (
                  <option key={a.id} value={a.name} />
                ))}
              </datalist>
            </div>
          </div>
          <div className="row">
            <div className="field">
              <label htmlFor="fFeat">Featuring / producent</label>
              <input
                id="fFeat"
                type="text"
                placeholder="opcjonalnie"
                value={featuring}
                onChange={(e) => setFeaturing(e.target.value)}
              />
            </div>
            <div className="field">
              <label htmlFor="fDate">Data premiery</label>
              <input
                id="fDate"
                type="date"
                value={date}
                onChange={(e) => setDate(e.target.value)}
              />
            </div>
          </div>
          <div className="field">
            <label htmlFor="fCtx">Kontekst kampanii</label>
            <textarea
              id="fCtx"
              placeholder="Teledysk, kampania TikTok/IG, konkurs UGC, livestream premierowy, wsparcie influencerów, trasa, album, którego singiel jest częścią…"
              value={ctx}
              onChange={(e) => setCtx(e.target.value)}
            />
            <span className="hint">
              Konkrety z planu marketingowego — redakcja Spotify je czyta.
            </span>
          </div>
          <div className="field">
            <label htmlFor="fLyrics">Tekst lub opis tekstu</label>
            <textarea
              id="fLyrics"
              placeholder="Opcjonalnie: wklej tekst albo napisz w dwóch zdaniach, o czym jest numer."
              value={lyrics}
              onChange={(e) => setLyrics(e.target.value)}
            />
          </div>
          <label className="check">
            <input
              type="checkbox"
              checked={forceResearch}
              onChange={(e) => setForceResearch(e.target.checked)}
            />
            <span>
              Odśwież research artysty, nawet jeśli ma pigułkę
              <br />
              <span className="hint">
                Wynik trafi jako propozycja — Twoja pigułka się nie zmieni.
              </span>
            </span>
          </label>
          <div className="row">
            <button className="btn primary big" disabled={running || preparing} onClick={start}>
              <Sparkles size={17} aria-hidden="true" />
              Analizuj
            </button>
            {running && (
              <button className="btn ghost" onClick={stop}>
                <Square size={17} aria-hidden="true" />
                Zatrzymaj
              </button>
            )}
          </div>
          {steps && <Steps steps={steps} />}
          {finished && (
            <div className="finished">
              Gotowe — analiza zapisana. Kroki oznaczone „–” zostały pominięte (opis przy kroku).
            </div>
          )}
          {error && <div className="err-box">{error}</div>}
        </div>

        <div className="panel sticky">
          {track?.analysis ? (
            <>
              <div className="result-head">
                <div>
                  <div className="eyebrow">{track.artist_name}</div>
                  <h2 style={{ marginTop: 6 }}>{track.title || "Bez tytułu"}</h2>
                </div>
              </div>
              <p className="hint">
                Analiza zapisana. Pitch pojawi się w kolejnym etapie; pigułkę artysty edytujesz w
                karcie Artyści.
              </p>
              <AnalysisView an={track.analysis} />
            </>
          ) : (
            <div className="empty-state">
              <div>
                <div className="eyebrow">Wynik</div>
                <h2 style={{ marginTop: 6 }}>Tu pojawi się analiza</h2>
              </div>
              <ul className="preview-list">
                <li>
                  <span className="pi">
                    <AudioLines size={18} aria-hidden="true" />
                  </span>
                  <div>
                    <b>Analiza odsłuchu</b>
                    <span className="hint">
                      BPM, tonacja, produkcja, struktura i momenty z timestampami.
                    </span>
                  </div>
                </li>
              </ul>
            </div>
          )}
        </div>
      </div>
    </section>
  );
}
