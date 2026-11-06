import { invoke } from "@tauri-apps/api/core";

export type SecretName = "gemini" | "anthropic";

export interface AppError {
  code: string;
  message: string;
}

/** Tauri zwraca błędy Rusta jako serializowany obiekt `{code, message}`. */
export function toAppError(e: unknown): AppError {
  if (e && typeof e === "object" && "message" in e) {
    const o = e as { code?: unknown; message: unknown };
    return { code: String(o.code ?? "other"), message: String(o.message) };
  }
  return { code: "other", message: String(e) };
}

export const api = {
  appVersion: () => invoke<string>("app_version"),
  setSecret: (name: SecretName, value: string) => invoke<void>("set_secret", { name, value }),
  hasSecret: (name: SecretName) => invoke<boolean>("has_secret", { name }),
  deleteSecret: (name: SecretName) => invoke<void>("delete_secret", { name }),
  getSettings: () => invoke<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listGeminiModels: () => invoke<string[]>("list_gemini_models"),
  testConnection: (args: { geminiModel?: string; claudeModel?: string; checkClaude: boolean }) =>
    invoke<{ gemini: string | null; claude: string | null }>("test_connection", args),
};
