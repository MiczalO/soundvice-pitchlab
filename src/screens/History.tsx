import { useEffect, useState } from "react";
import { ArrowDownUp } from "lucide-react";
import { api, toAppError, type ArtistRow, type HistoryRow } from "../lib/api";
import { fmtDateTime } from "../lib/format";
import { PITCH_LIMIT } from "../lib/meter";
import { useApp } from "../lib/store";

export function History() {
  const { notify, openTrack } = useApp();
  const [rows, setRows] = useState<HistoryRow[]>([]);
  const [artists, setArtists] = useState<ArtistRow[]>([]);
  const [artistId, setArtistId] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [newestFirst, setNewestFirst] = useState(true);
  const [loaded, setLoaded] = useState(false);

  useEffect(() => {
    api
      .listArtists()
      .then(setArtists)
      .catch(() => {});
  }, []);

  useEffect(() => {
    let alive = true;
    api
      .listHistory({ artistId, query, newestFirst })
      .then((r) => {
        if (alive) {
          setRows(r);
          setLoaded(true);
        }
      })
      .catch((e) => notify(toAppError(e).message));
    return () => {
      alive = false;
    };
  }, [artistId, query, newestFirst, notify]);

  const filtered = artistId != null || query.trim() !== "";

  return (
    <section className="view">
      <div className="view-head">
        <div className="eyebrow">Archiwum</div>
        <h1>Historia pitchy</h1>
      </div>
      <div className="panel">
        <div className="row">
          <div className="field" style={{ flex: "1 1 220px" }}>
            <label htmlFor="hQuery">Szukaj</label>
            <input
              id="hQuery"
              type="text"
              placeholder="tytuł lub artysta"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
            />
          </div>
          <div className="field" style={{ flex: "1 1 200px" }}>
            <label htmlFor="hArtist">Artysta</label>
            <select
              id="hArtist"
              value={artistId ?? ""}
              onChange={(e) => setArtistId(e.target.value ? Number(e.target.value) : null)}
            >
              <option value="">Wszyscy</option>
              {artists.map((a) => (
                <option key={a.id} value={a.id}>
                  {a.name}
                </option>
              ))}
            </select>
          </div>
          <button
            className="btn"
            style={{ alignSelf: "end" }}
            onClick={() => setNewestFirst((v) => !v)}
            aria-label="Zmień kolejność sortowania po dacie"
          >
            <ArrowDownUp size={17} aria-hidden="true" />
            {newestFirst ? "Najnowsze pierwsze" : "Najstarsze pierwsze"}
          </button>
        </div>
        <div className="tbl-wrap">
          <table className="hist">
            <thead>
              <tr>
                <th>Utwór</th>
                <th>Artysta</th>
                <th>Premiera</th>
                <th>Gatunek</th>
                <th>Znaki</th>
                <th>Utworzono</th>
              </tr>
            </thead>
            <tbody>
              {rows.map((r) => (
                <tr
                  key={r.track_id}
                  tabIndex={0}
                  onClick={() => openTrack(r.track_id)}
                  onKeyDown={(e) => e.key === "Enter" && openTrack(r.track_id)}
                >
                  <td>
                    <b>{r.title || "Bez tytułu"}</b>
                  </td>
                  <td>{r.artist_name}</td>
                  <td>{r.release_date ?? "—"}</td>
                  <td>{r.genre ?? "—"}</td>
                  <td className={`count${r.chars != null && r.chars > PITCH_LIMIT ? " over" : ""}`}>
                    {r.chars ?? "—"}
                  </td>
                  <td className="hint">{fmtDateTime(r.created_at)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </div>
        {loaded && rows.length === 0 && (
          <p className="empty">
            {filtered
              ? "Brak wyników dla tych filtrów."
              : "Jeszcze nic. Pierwszy pitch pojawi się tu po analizie."}
          </p>
        )}
      </div>
    </section>
  );
}
