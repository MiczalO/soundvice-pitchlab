import type { StepState } from "../lib/api";

export const STEP_DEFS: [string, string][] = [
  ["audio", "Przygotowanie pliku"],
  ["listen", "Gemini słucha i analizuje"],
  ["research", "Research o artyście"],
  ["pill", "Pigułka artysty"],
  ["pitch", "Pitch Spotify"],
];

export type StepMap = Record<string, { state: StepState | ""; sub: string }>;

/** `only`: pokaż tylko wybrane kroki (np. sam research w karcie Artyści). */
export function Steps({ steps, only }: { steps: StepMap; only?: string[] }) {
  return (
    <ol className="steps">
      {STEP_DEFS.filter(([key]) => !only || only.includes(key)).map(([key, label], i) => {
        const s = steps[key] ?? { state: "", sub: "" };
        return (
          <li key={key} className={s.state === "err" ? "err" : s.state}>
            <span className="dot">{i + 1}</span>
            <div>
              <div className="t">{label}</div>
              <div className="sub">{s.sub}</div>
            </div>
          </li>
        );
      })}
    </ol>
  );
}
