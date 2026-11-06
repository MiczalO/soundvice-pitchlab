CREATE TABLE artists (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  slug TEXT NOT NULL UNIQUE,
  pill TEXT NOT NULL DEFAULT '',
  manager_notes TEXT NOT NULL DEFAULT '',
  research_hints TEXT NOT NULL DEFAULT '',
  dossier TEXT NOT NULL DEFAULT '',
  created_at INTEGER NOT NULL,
  updated_at INTEGER NOT NULL,
  researched_at INTEGER
);

CREATE TABLE artist_sources (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
  title TEXT NOT NULL,
  url TEXT NOT NULL,
  fetched_at INTEGER NOT NULL
);

CREATE TABLE pill_proposals (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
  pill TEXT NOT NULL,
  dossier TEXT NOT NULL DEFAULT '',
  sources_json TEXT NOT NULL DEFAULT '[]',
  created_at INTEGER NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending'
    CHECK (status IN ('pending','accepted','appended','rejected'))
);

CREATE TABLE pill_history (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
  pill TEXT NOT NULL,
  changed_at INTEGER NOT NULL,
  change_type TEXT NOT NULL CHECK (change_type IN ('manual','research_accept','append','restore'))
);

CREATE TABLE tracks (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
  title TEXT NOT NULL DEFAULT '',
  featuring TEXT NOT NULL DEFAULT '',
  release_date TEXT,
  campaign_context TEXT NOT NULL DEFAULT '',
  lyrics TEXT NOT NULL DEFAULT '',
  audio_path TEXT,
  audio_hash TEXT,
  duration_s REAL,
  created_at INTEGER NOT NULL
);

CREATE TABLE analyses (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  model TEXT NOT NULL,
  json TEXT NOT NULL,
  created_at INTEGER NOT NULL
);

CREATE TABLE pitches (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
  text TEXT NOT NULL,
  text_pl TEXT NOT NULL DEFAULT '',
  form_json TEXT NOT NULL DEFAULT '{}',
  angle TEXT NOT NULL DEFAULT '',
  facts_used_json TEXT NOT NULL DEFAULT '[]',
  engine TEXT NOT NULL DEFAULT 'gemini',
  model TEXT NOT NULL DEFAULT '',
  is_manual_edit INTEGER NOT NULL DEFAULT 0,
  is_current INTEGER NOT NULL DEFAULT 1,
  created_at INTEGER NOT NULL
);

CREATE TABLE settings (
  key TEXT PRIMARY KEY,
  value TEXT NOT NULL
);

-- kind: 'rules' | 'examples'; aktualna wersja = najwyższy `version` danego rodzaju
CREATE TABLE rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT NOT NULL CHECK (kind IN ('rules','examples')),
  version INTEGER NOT NULL,
  body TEXT NOT NULL,
  created_at INTEGER NOT NULL,
  UNIQUE (kind, version)
);

-- cache URI plików wgranych do Gemini Files API (wygasają po ~48 h)
CREATE TABLE gemini_files (
  audio_hash TEXT PRIMARY KEY,
  file_uri TEXT NOT NULL,
  mime TEXT NOT NULL,
  expires_at INTEGER NOT NULL
);

CREATE INDEX idx_tracks_artist ON tracks(artist_id);
CREATE INDEX idx_pitches_track ON pitches(track_id);
CREATE INDEX idx_proposals_artist ON pill_proposals(artist_id, status);
CREATE INDEX idx_pill_history_artist ON pill_history(artist_id);
