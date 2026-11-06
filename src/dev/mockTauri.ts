/**
 * Podgląd interfejsu w zwykłej przeglądarce (tylko DEV, `npm run dev`): udaje backend Tauri
 * przykładowymi danymi, żeby można było oglądać i testować wygląd bez uruchamiania aplikacji.
 * W buildzie produkcyjnym ten plik nie jest dołączany.
 */
type Handler = (e: { event: string; id: number; payload: unknown }) => void;

const handlers = new Map<number, Handler>();
const byEvent = new Map<string, number[]>();
let seq = 1;

function emit(event: string, payload: unknown) {
  for (const id of byEvent.get(event) ?? []) handlers.get(id)?.({ event, id: seq++, payload });
}
const later = (ms: number, fn: () => void) => setTimeout(fn, ms);

const NOW = Date.now();
const DAY = 86_400_000;

const settings: Record<string, string> = {
  onboarded: "1",
  theme: "system",
  engine: "gemini",
  pitch_lang: "en",
  claude_model: "claude-opus-5-5",
  gemini_model: "gemini-3.6-flash",
  gemini_model_research: "gemini-3.6-flash",
  gemini_models_cache: JSON.stringify([
    "gemini-3.6-flash",
    "gemini-3.5-flash",
    "gemini-3.1-pro-preview",
    "gemini-2.5-pro",
  ]),
  gemini_model_status: JSON.stringify({
    "gemini-3.6-flash": "ok",
    "gemini-3.5-flash": "ok",
    "gemini-3.1-pro-preview": "quota",
    "gemini-2.5-pro": "retired",
  }),
};

const analysis = {
  primary_genre: "Hip-Hop / Rap",
  subgenres: ["Jersey Club Rap", "Pop Trap", "Dance-Pop"],
  genre_rationale: "Szybka stopa w stylu Jersey Club pod rapowaną, melodyjną nawijką.",
  bpm: 132,
  key: "B minor",
  energy: 8,
  moods: ["Energetic", "Party", "Confident"],
  instruments: ["808 bass", "Synths", "Hi-hats"],
  production_notes: "Skoczny rytm, głęboki bas 808 i jasne syntezatory; wokal mocno przetworzony.",
  vocals: {
    type: "melodic rap",
    language: "Polish",
    delivery: "lekki, rytmiczny flow",
    features: null,
  },
  lyrics_themes: ["klub", "sukces", "przyjaciele"],
  lyrics_summary: "Imprezowy numer o beztroskim wieczorze i świętowaniu sukcesu z ekipą.",
  hook: "Powtarzany refren „dymi to, dymi to”.",
  structure: [
    { t: "0:00", section: "intro" },
    { t: "0:07", section: "chorus" },
    { t: "0:22", section: "verse 1" },
    { t: "1:06", section: "verse 2" },
    { t: "2:12", section: "outro" },
  ],
  standout_moments: [{ t: "1:06", desc: "przyspieszenie flow w drugiej zwrotce" }],
  sound_alikes: [],
  explicit_content: false,
  editorial_angle: "Klubowy rytm jersey club w polskim rapie.",
  opening_lines: "Tak to się robi teraz, mała, kręć to teraz",
  first_vocal_at: "0:07",
  heard_audio_s: 143.6,
};

const form = {
  genres: ["Hip-Hop", "Pop Trap", "Jersey Club"],
  moods: ["Energetic", "Party", "Confident"],
  styles: ["Polish Hip-Hop", "Pop Rap"],
  instruments: ["808 Bass", "Synthesizers", "Drum Kit"],
  culture: "Polish Hip-Hop",
  song_type: "Original",
  vocal_language: "Polish",
};

const pitches = [
  {
    id: 3,
    text: '"DYMI2" is the new single by Test Artysta, blending energetic pop-trap with a vibrant Jersey Club rhythm. Produced by Jan Kowalski, the track pairs deep 808 bass and bouncy kicks with bright synths and heavily processed vocals. Lyrically, it centers on a carefree night out and celebrating success with friends. It\'s a crisp, party-ready cut with an infectious hook.',
    text_pl:
      "„DYMI2” to nowy singiel Test Artysta, łączący energetyczny pop-trap z żywym rytmem Jersey Club.",
    form,
    angle: "Klubowy rytm jako główny wyróżnik.",
    facts_used: ["analiza: tempo i rytm Jersey Club", "pole: prod. Jan Kowalski"],
    engine: "gemini",
    model: "gemini-3.6-flash",
    is_manual_edit: false,
    is_current: true,
    created_at: NOW - 3_600_000,
  },
  {
    id: 2,
    text: "A club-ready pop-trap track with a Jersey Club bounce and a repeating, chant-like hook.",
    text_pl: "",
    form,
    angle: "",
    facts_used: [],
    engine: "gemini",
    model: "gemini-3.5-flash",
    is_manual_edit: true,
    is_current: false,
    created_at: NOW - 2 * 3_600_000,
  },
];

const track = {
  id: 1,
  artist_id: 1,
  artist_name: "Test Artysta",
  title: "DYMI2",
  featuring: "prod. Jan Kowalski",
  release_date: "2026-12-05",
  campaign_context: "Teledysk, kampania TikTok w dniu premiery.",
  lyrics: "",
  audio_path: null,
  duration_s: 144,
  created_at: NOW - 3_700_000,
  analysis,
  pitches,
};

const PILL =
  "KIM JEST:\nRaperka z Opola, debiut 2019.\n\nBRZMIENIE I TEMATY:\nPop-trap z elementami jersey club.\n\nKAMIENIE MILOWE:\nZłota płyta ZPAV za singiel „Noc” (2022).\n\nNIEPEWNE / NIE UŻYWAĆ:\nData urodzenia [?]";

const artists = [
  { id: 1, name: "Test Artysta", has_pill: true, has_proposal: true, updated_at: NOW - DAY },
  { id: 2, name: "Mata Mock", has_pill: false, has_proposal: false, updated_at: NOW - 3 * DAY },
];

const detail = {
  id: 1,
  name: "Test Artysta",
  pill: PILL,
  manager_notes: "Nie wspominać o poprzedniej wytwórni.",
  research_hints: "raperka z Opola, nie mylić z DJ-em",
  dossier: "Dossier z researchu…",
  updated_at: NOW - DAY,
  researched_at: NOW - DAY,
  sources: [{ title: "Wikipedia", url: "https://pl.wikipedia.org" }],
  proposal: {
    id: 1,
    pill: PILL.replace("debiut 2019", "debiut 2019, album w 2021").replace(
      "Pop-trap",
      "Pop-trap i jersey club",
    ),
    dossier: "",
    sources: [],
    created_at: NOW - 3_600_000,
  },
  history: [
    { id: 1, pill: "Poprzednia wersja pigułki…", changed_at: NOW - 2 * DAY, change_type: "manual" },
  ],
  tracks: [{ id: 1, title: "DYMI2", created_at: NOW - 3_700_000 }],
};

const rules = {
  rules:
    "CEL: pitch do redakcji Spotify…\n\nLIMIT: maksymalnie 500 znaków ze spacjami.\n\nSTRUKTURA — zwykle 4 zdania…",
  examples: "Przykładowy pitch 1.\n---\nPrzykładowy pitch 2.",
  lang: "en",
  default_rules: "CEL: pitch do redakcji Spotify…",
  rules_versions: [
    { id: 3, version: 3, body: "CEL: pitch do redakcji Spotify…\n\nLIMIT…", created_at: NOW - DAY },
    { id: 1, version: 1, body: "Pierwsza wersja zasad", created_at: NOW - 5 * DAY },
  ],
  examples_versions: [{ id: 2, version: 2, body: "Przykładowy pitch 1.", created_at: NOW - DAY }],
};

const historyRows = [
  {
    track_id: 1,
    title: "DYMI2",
    artist_id: 1,
    artist_name: "Test Artysta",
    release_date: "2026-12-05",
    genre: "Hip-Hop / Rap",
    chars: 468,
    created_at: NOW - 3_700_000,
  },
  {
    track_id: 2,
    title: "RETRO",
    artist_id: 2,
    artist_name: "Mata Mock",
    release_date: null,
    genre: "Pop",
    chars: 512,
    created_at: NOW - 2 * DAY,
  },
];

function simulateJob(steps: [string, number][], track_id: number | null) {
  let t = 100;
  for (const [step, ms] of steps) {
    later(t, () => emit("job-step", { job_id: 9, step, state: "active", sub: "pracuję…" }));
    t += ms;
    later(t, () => emit("job-step", { job_id: 9, step, state: "done", sub: "gotowe" }));
  }
  later(t + 100, () => emit("job-done", { job_id: 9, track_id, cancelled: false, error: null }));
  return 9;
}

const peaks = Array.from(
  { length: 600 },
  (_, i) => 0.25 + 0.7 * Math.abs(Math.sin(i / 14) * Math.cos(i / 53)),
);

const commands: Record<string, (a: Record<string, unknown>) => unknown> = {
  app_version: () => "0.1.0-dev",
  has_secret: () => true,
  set_secret: () => null,
  delete_secret: () => null,
  get_settings: () => ({ ...settings }),
  set_setting: (a) => void (settings[String(a.key)] = String(a.value)),
  list_gemini_models: () => JSON.parse(settings.gemini_models_cache),
  probe_gemini_models: () =>
    JSON.parse(settings.gemini_models_cache).map((m: string) => ({
      model: m,
      status: JSON.parse(settings.gemini_model_status)[m],
    })),
  test_connection: () => ({ gemini: "OK", claude: null }),
  list_artists: () => artists,
  get_artist: (a) =>
    a.id === 1
      ? detail
      : {
          ...detail,
          id: 2,
          name: "Mata Mock",
          pill: "",
          proposal: null,
          history: [],
          tracks: [],
          sources: [],
          dossier: "",
        },
  create_artist: () => 3,
  update_artist: () => null,
  delete_artist: () => null,
  accept_proposal: () => null,
  reject_proposal: () => null,
  restore_pill: () => null,
  start_research: () =>
    simulateJob(
      [
        ["research", 900],
        ["pill", 700],
      ],
      null,
    ),
  start_pill_from_dossier: () => simulateJob([["pill", 900]], null),
  prepare_audio: () => ({
    path: "C:/mock/Master.wav",
    hash: "abc",
    size: 38_103_102,
    mime: "audio/wav",
    file_name: "Test Artysta - DYMI2_v1.wav",
    waveform: { peaks, duration_s: 144 },
  }),
  start_analysis: () =>
    simulateJob(
      [
        ["audio", 600],
        ["listen", 1200],
        ["research", 400],
        ["pill", 300],
        ["pitch", 1000],
      ],
      1,
    ),
  start_pitch: () => simulateJob([["pitch", 1200]], 1),
  cancel_job: () => null,
  get_track: () => track,
  save_pitch_edit: () => 4,
  restore_pitch: () => null,
  add_to_examples: () => null,
  list_history: () => historyRows,
  get_rules: () => rules,
  save_rules: () => true,
  restore_rule_version: () => null,
  reset_default_rules: () => null,
  import_html_json: () => ({
    artists_new: 2,
    artists_merged: 1,
    tracks: 5,
    tracks_skipped: 0,
    pitches: 9,
    proposals: 1,
    rules_updated: true,
    examples_updated: false,
    lang_set: true,
  }),
  export_backup: () => ({ path: "C:/mock/kopia.zip", audio_files: 0, size: 17_779 }),
  restore_backup: () => null,
  get_data_dir: () => "C:/mock",
  "plugin:updater|check": () => null,
  "plugin:dialog|open": () => "C:/mock/plik",
  "plugin:dialog|save": () => "C:/mock/kopia.zip",
  "plugin:opener|reveal_item_in_dir": () => null,
  "plugin:opener|open_url": () => null,
  "plugin:path|resolve_directory": () => "C:/mock",
  "plugin:path|join": (a) => (a.paths as string[]).join("/"),
};

const w = window as unknown as Record<string, unknown>;
w.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener: () => {} };
w.__TAURI_INTERNALS__ = {
  metadata: {
    currentWindow: { label: "main" },
    currentWebview: { windowLabel: "main", label: "main" },
  },
  convertFileSrc: (p: string) => p,
  transformCallback: (cb: Handler) => {
    const id = seq++;
    handlers.set(id, cb);
    return id;
  },
  invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
    if (cmd === "plugin:event|listen") {
      const event = String(args.event);
      const id = Number(args.handler);
      byEvent.set(event, [...(byEvent.get(event) ?? []), id]);
      return id;
    }
    if (cmd === "plugin:event|unlisten") return null;
    const fn = commands[cmd];
    if (!fn) throw { code: "other", message: `Mock: brak komendy ${cmd}` };
    await new Promise((r) => setTimeout(r, 60));
    return fn(args);
  },
};
console.info("[pitchLAB] tryb podglądu DEV: backend Tauri jest udawany");

export {};
