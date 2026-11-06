import { LOGO_INNER, LOGO_OUTER } from "../assets/brand/logoPaths";

/**
 * Oficjalny znak Soundvice (z assets/brand/BW_1.svg). Kolor konturu bierze z `currentColor`,
 * a wypełnienia z `--mark-inner`, dzięki czemu pasuje do każdego tła i motywu.
 */
export function LogoMark({ size = 40, title }: { size?: number; title?: string }) {
  return (
    <svg
      viewBox="0 0 1024 1024"
      width={size}
      height={size}
      role={title ? "img" : undefined}
      aria-label={title}
      aria-hidden={title ? undefined : true}
      focusable="false"
    >
      <path d={LOGO_OUTER} fill="currentColor" />
      {LOGO_INNER.map((d, i) => (
        <path key={i} d={d} fill="var(--mark-inner, #0b0c0e)" />
      ))}
    </svg>
  );
}
