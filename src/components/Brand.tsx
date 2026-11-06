const BARS = [6, 14, 9, 16, 5];

/**
 * Znak i logotyp „Soundvice pitchLAB”. Znak jest jednym komponentem — podmiana na oficjalne
 * logo (SVG) wymaga zmiany tylko tutaj. `onPage`: wariant na tle strony (kreator), domyślnie
 * wariant na ciemnym pasku bocznym.
 */
export function Brand({ onPage = false }: { onPage?: boolean }) {
  return (
    <span className={`brand${onPage ? " on-page" : ""}`} aria-label="Soundvice pitchLAB">
      <span className="brand-mark" aria-hidden="true">
        <span>
          {BARS.map((h, i) => (
            <i key={i} style={{ height: h }} />
          ))}
        </span>
      </span>
      <span className="brand-text">
        <b>Soundvice</b>
        <em>
          pitch<strong>LAB</strong>
        </em>
      </span>
    </span>
  );
}
