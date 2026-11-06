import { useState } from "react";
import { api, toAppError } from "../lib/api";
import { useApp } from "../lib/store";

/** Pobiera listę modeli Gemini i zapamiętuje ją w ustawieniach (cache). */
export function useModels() {
  const { settings, saveSetting } = useApp();
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const models = parseModels(settings.gemini_models_cache);
  const load = async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await api.listGeminiModels();
      await saveSetting("gemini_models_cache", JSON.stringify(list));
      // Pierwsze uruchomienie: ustaw domyślnie najnowszy „pro” (pierwszy na liście).
      if (list.length && !settings.gemini_model) await saveSetting("gemini_model", list[0]);
      if (list.length && !settings.gemini_model_research)
        await saveSetting("gemini_model_research", list[0]);
      return list;
    } catch (e) {
      setError(toAppError(e).message);
      return [];
    } finally {
      setLoading(false);
    }
  };
  return { models, load, loading, error };
}

function parseModels(raw: string | undefined): string[] {
  try {
    const v = JSON.parse(raw || "[]");
    return Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : [];
  } catch {
    return [];
  }
}
