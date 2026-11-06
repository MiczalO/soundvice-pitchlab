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

/** Zabezpieczenie UI: gdyby backend nie odpowiedział (np. restart w trybie dev), przycisk nie wisi w nieskończoność. */
export function withTimeout<T>(p: Promise<T>, ms: number): Promise<T> {
  return new Promise<T>((resolve, reject) => {
    const t = setTimeout(
      () =>
        reject({
          code: "timeout",
          message:
            "Brak odpowiedzi z aplikacji. Spróbuj ponownie; jeśli się powtarza, zrestartuj aplikację.",
        }),
      ms,
    );
    p.then(
      (v) => {
        clearTimeout(t);
        resolve(v);
      },
      (e) => {
        clearTimeout(t);
        reject(e);
      },
    );
  });
}

export const api = {
  appVersion: () => invoke<string>("app_version"),
  setSecret: (name: SecretName, value: string) => invoke<void>("set_secret", { name, value }),
  hasSecret: (name: SecretName) => invoke<boolean>("has_secret", { name }),
  deleteSecret: (name: SecretName) => invoke<void>("delete_secret", { name }),
  getSettings: () => invoke<Record<string, string>>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  listGeminiModels: () => withTimeout(invoke<string[]>("list_gemini_models"), 60_000),
  testConnection: (args: { geminiModel?: string; claudeModel?: string; checkClaude: boolean }) =>
    withTimeout(
      invoke<{ gemini: string | null; claude: string | null }>("test_connection", args),
      150_000,
    ),
  prepareAudio: (path: string) => invoke<PreparedAudio>("prepare_audio", { path }),
  startAnalysis: (params: AnalysisParams) => invoke<number>("start_analysis", { params }),
  cancelJob: (jobId: number) => invoke<void>("cancel_job", { jobId }),
  getTrack: (trackId: number) => invoke<TrackView | null>("get_track", { trackId }),
};

export interface PreparedAudio {
  path: string;
  hash: string;
  size: number;
  mime: string;
  file_name: string;
  waveform: { peaks: number[]; duration_s: number } | null;
}

export interface AnalysisParams {
  filePath: string;
  hash: string;
  mime: string;
  fileName: string;
  size: number;
  durationS: number | null;
  title: string;
  artist: string;
  featuring: string;
  releaseDate: string | null;
  campaignContext: string;
  lyrics: string;
}

export interface Analysis {
  primary_genre: string;
  subgenres: string[];
  genre_rationale: string;
  bpm: number | null;
  key: string | null;
  energy: number | null;
  moods: string[];
  instruments: string[];
  production_notes: string;
  vocals: { type: string; language: string; delivery: string; features: string | null };
  lyrics_themes: string[];
  lyrics_summary: string;
  hook: string;
  structure: { t: string; section: string }[];
  standout_moments: { t: string; desc: string }[];
  sound_alikes: string[];
  explicit_content: boolean | null;
  editorial_angle: string;
}

export interface TrackView {
  id: number;
  artist_id: number;
  artist_name: string;
  title: string;
  featuring: string;
  release_date: string | null;
  campaign_context: string;
  audio_path: string | null;
  duration_s: number | null;
  created_at: number;
  analysis: Analysis | null;
}

export type StepState = "active" | "done" | "skip" | "err";
export interface StepEvent {
  job_id: number;
  step: string;
  state: StepState;
  sub: string | null;
}
export interface DoneEvent {
  job_id: number;
  track_id: number | null;
  cancelled: boolean;
  error: AppError | null;
}
