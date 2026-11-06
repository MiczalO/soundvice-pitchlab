import type { StepState } from "../lib/api";

export const STEP_DEFS: [string, string][] = [
  ["audio", "Przygotowanie pliku"],
  ["listen", "Gemini słucha i analizuje"],
  ["research", "Research o artyście"],
  ["pill", "Pigułka artysty"],
  ["pitch", "Pitch Spotify"],
];

export type StepMap = Record<string, { state: StepState | ""; sub: string }>;

export function Steps({ steps }: { steps: StepMap }) {
  return (
    <ol className="steps">
      {STEP_DEFS.map(([key, label], i) => {
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
