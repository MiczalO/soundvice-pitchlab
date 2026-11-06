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
  probeGeminiModels: (models: string[]) =>
    withTimeout(
      invoke<{ model: string; status: string }[]>("probe_gemini_models", { models }),
      90_000,
    ),
  testConnection: (args: { geminiModel?: string; claudeModel?: string; checkClaude: boolean }) =>
    withTimeout(
      invoke<{ gemini: string | null; claude: string | null }>("test_connection", args),
      150_000,
    ),
  prepareAudio: (path: string) => invoke<PreparedAudio>("prepare_audio", { path }),
  startAnalysis: (params: AnalysisParams) => invoke<number>("start_analysis", { params }),
  listArtists: () => invoke<ArtistRow[]>("list_artists"),
  getArtist: (id: number) => invoke<ArtistDetail | null>("get_artist", { id }),
  createArtist: (name: string) => invoke<number>("create_artist", { name }),
  updateArtist: (a: { id: number; pill: string; managerNotes: string; researchHints: string }) =>
    invoke<void>("update_artist", a),
  deleteArtist: (id: number) => invoke<void>("delete_artist", { id }),
  acceptProposal: (proposalId: number, mode: "replace" | "append") =>
    invoke<void>("accept_proposal", { proposalId, mode }),
  rejectProposal: (proposalId: number) => invoke<void>("reject_proposal", { proposalId }),
  restorePill: (historyId: number) => invoke<void>("restore_pill", { historyId }),
  startResearch: (artistId: number) => invoke<number>("start_research", { artistId }),
  startPillFromDossier: (artistId: number, dossier: string) =>
    invoke<number>("start_pill_from_dossier", { artistId, dossier }),
  startPitch: (args: {
    trackId: number;
    mode: "fresh" | "new" | "revise";
    feedback?: string;
    currentText?: string;
  }) => invoke<number>("start_pitch", args),
  savePitchEdit: (trackId: number, text: string) =>
    invoke<number>("save_pitch_edit", { trackId, text }),
  restorePitch: (pitchId: number) => invoke<void>("restore_pitch", { pitchId }),
  addToExamples: (text: string) => invoke<void>("add_to_examples", { text }),
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
  forceResearch: boolean;
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
  pitches: PitchView[];
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

export interface ArtistRow {
  id: number;
  name: string;
  has_pill: boolean;
  has_proposal: boolean;
  updated_at: number;
}
export interface SourceRow {
  title: string;
  url: string;
}
export interface Proposal {
  id: number;
  pill: string;
  dossier: string;
  sources: SourceRow[];
  created_at: number;
}
export interface HistoryRow {
  id: number;
  pill: string;
  changed_at: number;
  change_type: "manual" | "research_accept" | "append" | "restore";
}
export interface TrackBrief {
  id: number;
  title: string;
  created_at: number;
}
export interface ArtistDetail {
  id: number;
  name: string;
  pill: string;
  manager_notes: string;
  research_hints: string;
  dossier: string;
  updated_at: number;
  researched_at: number | null;
  sources: SourceRow[];
  proposal: Proposal | null;
  history: HistoryRow[];
  tracks: TrackBrief[];
}

export interface PitchForm {
  genres: string[];
  moods: string[];
  styles: string[];
  instruments: string[];
  culture: string;
  song_type: string;
  vocal_language: string;
}
export interface PitchView {
  id: number;
  text: string;
  text_pl: string;
  form: Partial<PitchForm> | null;
  angle: string;
  facts_used: string[];
  engine: string;
  model: string;
  is_manual_edit: boolean;
  is_current: boolean;
  created_at: number;
}
