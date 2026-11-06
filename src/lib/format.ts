export const fmtTime = (s: number | null | undefined): string =>
  s == null || !Number.isFinite(s)
    ? "—"
    : `${Math.floor(s / 60)}:${String(Math.floor(s % 60)).padStart(2, "0")}`;

export const fmtSize = (b: number): string =>
  b > 1048576 ? `${(b / 1048576).toFixed(1)} MB` : `${Math.max(1, Math.round(b / 1024))} KB`;

/** Tytuł z nazwy pliku: bez rozszerzenia i numeru ścieżki ("01 - Retro.wav" → "Retro"). */
export const titleFromFile = (name: string): string =>
  name
    .replace(/\.[^.]+$/, "")
    .replace(/^\d+[\s._-]+/, "")
    .replace(/_+/g, " ")
    .trim();

export const fmtDateTime = (ms: number | null | undefined): string =>
  ms
    ? new Date(ms).toLocaleString("pl-PL", {
        day: "2-digit",
        month: "2-digit",
        year: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      })
    : "—";
