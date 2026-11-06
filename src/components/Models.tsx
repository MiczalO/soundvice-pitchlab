import { useState } from "react";
import { api, toAppError } from "../lib/api";
import { useApp } from "../lib/store";

export type ModelStatus = "ok" | "retired" | "quota" | "overloaded" | "timeout" | "error";
export type StatusMap = Record<string, ModelStatus>;

export const STATUS_LABEL: Record<ModelStatus, string> = {
  ok: "działa",
  retired: "wycofany dla nowych kont",
  quota: "brak przydziału w Twoim planie",
  overloaded: "chwilowo przeciążony",
  timeout: "brak odpowiedzi",
  error: "błąd",
};

export const modelLabel = (m: string, status: StatusMap): string =>
  status[m] ? `${m} — ${STATUS_LABEL[status[m]]}` : m;

/** Pierwszy działający model z listy (kolejność = ranking), potem przeciążony, potem cokolwiek. */
export function bestModel(list: string[], status: StatusMap): string | undefined {
  return (
    list.find((m) => status[m] === "ok") ?? list.find((m) => status[m] === "overloaded") ?? list[0]
  );
}

function parseJson<T>(raw: string | undefined, fallback: T): T {
  try {
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

/**
 * Lista modeli Gemini + sondowanie, które z nich naprawdę odpowiadają dla tego klucza
 * (lista z API zawiera też modele wycofane lub bez przydziału w danym planie).
 */
export function useModels() {
  const { settings, saveSetting } = useApp();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const rawList = parseJson<unknown>(settings.gemini_models_cache, []);
  const models = Array.isArray(rawList)
    ? rawList.filter((x): x is string => typeof x === "string")
    : [];
  const status = parseJson<StatusMap>(settings.gemini_model_status, {});

  /** `force`: wybierz najlepszy działający model niezależnie od dotychczasowego wyboru. */
  const load = async (force: boolean) => {
    setLoading(true);
    setError(null);
    try {
      const list = await api.listGeminiModels();
      const probes = await api.probeGeminiModels(list);
      const st = Object.fromEntries(probes.map((p) => [p.model, p.status])) as StatusMap;
      await saveSetting("gemini_models_cache", JSON.stringify(list));
      await saveSetting("gemini_model_status", JSON.stringify(st));
      const current = useApp.getState().settings;
      const best = bestModel(list, st);
      for (const key of ["gemini_model", "gemini_model_research"]) {
        const cur = current[key];
        if (best && (force || !cur || !list.includes(cur))) await saveSetting(key, best);
      }
      return list;
    } catch (e) {
      setError(toAppError(e).message);
      return [];
    } finally {
      setLoading(false);
    }
  };
  return { models, status, load, loading, error };
}
