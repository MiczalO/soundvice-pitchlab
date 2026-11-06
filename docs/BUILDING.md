# Budowanie i wydawanie (dla programistów)

Stos: **Tauri 2** (Rust) + **React + TypeScript + Vite**, baza **SQLite** (`rusqlite`), klucze w sejfie systemowym (`keyring`).
Wywołania Gemini i Claude idą wyłącznie z warstwy Rust.

## Wymagania

- Node.js 22+, npm
- Rust (stable) i narzędzia Tauri: <https://tauri.app/start/prerequisites/>
  - Windows: Visual Studio Build Tools (Desktop C++) i WebView2 (jest w Windows 11)
  - macOS: Xcode Command Line Tools

## Praca lokalna

```bash
npm install
npm run tauri dev      # aplikacja z hot-reload
npm run dev            # sam frontend w przeglądarce (http://localhost:1420) z udawanym backendem
```

Tryb `npm run dev` w zwykłej przeglądarce używa `src/dev/mockTauri.ts` (przykładowe dane, bez Gemini). Ten plik jest
dołączany **tylko** w trybie DEV i nie trafia do buildu produkcyjnego.

### Testy i jakość (to samo robi CI)

```bash
npm run typecheck && npm run lint && npm run format:check && npm test
cd src-tauri
cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

## Struktura

```
src/                       frontend (React)
  screens/                 Run, Artists, History, Rules, Settings, Wizard
  components/              Player, TrackResult, Steps, UpdatePanel, DataPanel, LogoMark, ...
  lib/                     api.ts (komendy Tauri), store.ts (Zustand), meter.ts, diff.ts, format.ts
  styles/app.css           system designu (tokeny w :root; ciemny główny, jasny drugi)
  dev/mockTauri.ts         udawany backend do podglądu w przeglądarce (tylko DEV)
src-tauri/src/
  ai/                      gemini.rs, anthropic.rs, prompts.rs, resilient.rs (ponawianie + zapasowe modele), ...
  db/                      migracje SQL, artists, pitches, rules, history, repo
  pipeline.rs              odsłuch → research/pigułka → pitch
  research.rs, pitch.rs    kroki pipeline
  audio.rs                 import mastera, waveform (symphonia)
  backup.rs, import.rs     kopia ZIP i import z prototypu HTML
  secrets.rs               klucze API w sejfie systemowym
assets/brand/              logo (SVG) i źródło ikony aplikacji
legacy/                    prototyp HTML (źródło prawdy dla promptów)
```

- **Dane użytkownika:** katalog danych aplikacji (`pl.repitch.soundvice-pitchlab`): `pitchlab.sqlite3`, `audio/`.
- **Migracje:** `src-tauri/src/db/migrations/*.sql`, wersjonowane (`PRAGMA user_version`). Zwiększając schemat,
  dodaj migrację i zaktualizuj `SCHEMA_VERSION` w `db/mod.rs` (test pilnuje zgodności).
- **Sekrety:** klucze API nie występują w bazie, logach ani kopiach. Ustawienia mają białą listę kluczy (`settings.rs`).
- **CSP:** produkcyjny CSP jest w `tauri.conf.json` (zasoby tylko z aplikacji, audio przez protokół `asset`).

## Marka (logo i ikona)

- Logo: `assets/brand/BW_1.svg` i `WB_1.svg`; w aplikacji komponent `LogoMark` (ścieżki w `src/assets/brand/logoPaths.ts`).
- Ikona: `assets/brand/app-icon.svg` (kafelek 1024×1024 z zaokrągleniem). Po zmianie wygeneruj ikony:

```bash
npx tauri icon assets/brand/app-icon.svg
# usuń niepotrzebne foldery mobilne: src-tauri/icons/android i src-tauri/icons/ios
```

## Wydawanie wersji

1. Ustaw tę samą wersję w `package.json`, `src-tauri/tauri.conf.json` i `src-tauri/Cargo.toml`.
2. Zatwierdź zmiany i wypchnij tag:

   ```bash
   git tag v0.2.0
   git push origin v0.2.0
   ```

3. Workflow **Release** (`.github/workflows/release.yml`) zbuduje instalatory: Windows (NSIS `.exe` + `.msi`) oraz macOS
   (`.dmg` dla Apple Silicon i Intel) i utworzy **szkic wydania** (draft).
4. Sprawdź instalatory i kliknij **Publish release**. Dopiero opublikowane wydanie jest widoczne dla auto-aktualizacji
   (`latest.json` jest pobierany z `releases/latest/download/`).

Workflow przerwie się, jeśli wersja w tagu nie zgadza się z plikami projektu albo jeśli w `tauri.conf.json` został
placeholder `OWNER`.

### Pierwsza konfiguracja repozytorium

- Repozytorium ma się nazywać **`MiczalO/soundvice-pitchlab`** — taki adres jest w `plugins.updater.endpoints`
  (`src-tauri/tauri.conf.json`). Inna nazwa lub właściciel wymaga zmiany tego adresu.
- W repozytorium: *Settings → Actions → General → Workflow permissions → Read and write permissions*.

## Aktualizacje w aplikacji (podpisywanie aktualizacji)

Aktualizacje są podpisane kluczem Tauri. Klucz **publiczny** jest w `tauri.conf.json` (`plugins.updater.pubkey`),
klucz **prywatny** nie może trafić do repozytorium.

```bash
# wygenerowanie nowej pary kluczy (zalecane z hasłem do produkcji)
npx tauri signer generate -w ~/.tauri/soundvice-pitchlab.key
```

- Wklej zawartość pliku `.key.pub` do `plugins.updater.pubkey`.
- Dodaj sekrety GitHub: `TAURI_SIGNING_PRIVATE_KEY` (zawartość pliku `.key`) i `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
- **Zgubienie klucza lub hasła = brak możliwości wydawania aktualizacji** istniejącym instalacjom. Zrób jego kopię.
- Zmiana pary kluczy po pierwszym publicznym wydaniu wymaga, żeby użytkownicy zainstalowali nową wersję ręcznie.

Bez sekretu `TAURI_SIGNING_PRIVATE_KEY` workflow zbuduje instalatory, ale bez plików aktualizacji (ostrzeżenie w logu).

## Podpisywanie kodu (opcjonalne)

Wszystkie sekrety są opcjonalne: bez nich powstają **niepodpisane** instalatory (ostrzeżenia SmartScreen/Gatekeeper opisane
w README użytkownika).

| Sekret | Do czego |
| --- | --- |
| `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | podpis aktualizacji w aplikacji (patrz wyżej) |
| `WINDOWS_CERTIFICATE`, `WINDOWS_CERTIFICATE_PASSWORD` | certyfikat podpisu kodu Windows: plik `.pfx` w base64 i hasło |
| `APPLE_CERTIFICATE`, `APPLE_CERTIFICATE_PASSWORD` | certyfikat „Developer ID Application” (`.p12` w base64) i hasło |
| `APPLE_SIGNING_IDENTITY` | np. `Developer ID Application: Nazwa (TEAMID)` |
| `APPLE_ID`, `APPLE_PASSWORD`, `APPLE_TEAM_ID` | notaryzacja (hasło aplikacji z appleid.apple.com) |

Przygotowanie certyfikatu do sekretu (base64):

```powershell
# Windows (PowerShell)
[Convert]::ToBase64String([IO.File]::ReadAllBytes("certyfikat.pfx")) | Set-Clipboard
```

```bash
# macOS / Linux
base64 -i certyfikat.p12 | pbcopy
```

## Rozwiązywanie problemów z builderem

- **Brak WiX (MSI)** przy lokalnym buildzie: Tauri pobiera go przy pierwszym użyciu (potrzebny internet).
- **`tauri::generate_context!` nie widzi `dist`:** najpierw `npm run build` (CI robi to przed Rustem).
- **Ostrzeżenie `Failed to load a font face ... malformed font`** podczas `tauri icon` pochodzi z czcionek systemowych
  i jest nieszkodliwe.
