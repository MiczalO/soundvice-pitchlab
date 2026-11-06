import { LogoMark } from "./LogoMark";

/**
 * Znak i logotyp „Soundvice pitchLAB”. `onPage`: wariant na tle strony (kreator),
 * domyślnie wariant na ciemnym pasku bocznym.
 */
export function Brand({ onPage = false }: { onPage?: boolean }) {
  return (
    <span className={`brand${onPage ? " on-page" : ""}`} aria-label="Soundvice pitchLAB">
      <span className="brand-mark" aria-hidden="true">
        <LogoMark size={40} />
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
