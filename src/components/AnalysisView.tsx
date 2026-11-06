import type { Analysis } from "../lib/api";

const Chips = ({ items }: { items: string[] }) =>
  items.length ? (
    <div className="chips">
      {items.map((x) => (
        <span className="chip" key={x}>
          {x}
        </span>
      ))}
    </div>
  ) : (
    <span className="hint">—</span>
  );

/** Pełna analiza odsłuchu (port sekcji „Analiza odsłuchu” z prototypu). */
export function AnalysisView({ an }: { an: Analysis }) {
  const vocals = [an.vocals?.type, an.vocals?.language, an.vocals?.delivery]
    .filter(Boolean)
    .join(" · ");
  return (
    <dl className="kv" style={{ marginTop: 10 }}>
      <dt>Gatunek</dt>
      <dd>
        <b>{an.primary_genre || "—"}</b>
        {an.subgenres?.length ? ` · ${an.subgenres.join(", ")}` : ""}
        <div className="hint">{an.genre_rationale}</div>
      </dd>
      <dt>Tempo / tonacja</dt>
      <dd>
        {an.bpm != null ? Math.round(an.bpm) : "—"} BPM · {an.key || "—"} · energia{" "}
        {an.energy ?? "—"}/10
      </dd>
      <dt>Nastroje</dt>
      <dd>
        <Chips items={an.moods ?? []} />
      </dd>
      <dt>Instrumenty</dt>
      <dd>
        <Chips items={an.instruments ?? []} />
      </dd>
      <dt>Produkcja</dt>
      <dd>{an.production_notes || "—"}</dd>
      <dt>Wokal</dt>
      <dd>{vocals || "—"}</dd>
      <dt>Tekst</dt>
      <dd>
        {an.lyrics_summary || "—"}
        <div style={{ marginTop: 6 }}>
          <Chips items={an.lyrics_themes ?? []} />
        </div>
      </dd>
      <dt>Hook</dt>
      <dd>{an.hook || "—"}</dd>
      <dt>Struktura</dt>
      <dd>
        {an.structure?.length
          ? an.structure.map((s, i) => (
              <span key={i}>
                {i > 0 && " · "}
                <span className="ts">{s.t}</span> {s.section}
              </span>
            ))
          : "—"}
      </dd>
      <dt>Momenty</dt>
      <dd>
        {an.standout_moments?.length
          ? an.standout_moments.map((m, i) => (
              <div key={i}>
                <span className="ts">{m.t}</span> {m.desc}
              </div>
            ))
          : "—"}
      </dd>
      <dt>Brzmi jak</dt>
      <dd>{an.sound_alikes?.join("; ") || "—"}</dd>
      <dt>Kąt redakcyjny</dt>
      <dd>{an.editorial_angle || "—"}</dd>
    </dl>
  );
}
