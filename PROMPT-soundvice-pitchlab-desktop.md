# Soundvice pitchLAB — aplikacja desktopowa

> Prompt dla Claude Code. Umieść w pustym folderze projektu razem z plikiem `legacy/soundvice-pitchlab.html` (obecna wersja HTML — źródło prawdy dla logiki, promptów i wyglądu), uruchom `claude` i wklej treść poniżej.

---

## Zadanie

Zbuduj **Soundvice pitchLAB** jako pełnoprawną aplikację desktopową (Windows + macOS), którą da się spakować do instalatora i wysłać innej osobie, a ona po instalacji i wpisaniu własnego klucza API ma wszystko działające. To port i rozwinięcie istniejącego prototypu `legacy/soundvice-pitchlab.html` — przeczytaj go w całości, zanim zaczniesz planować. Funkcje, prompty do modeli, domyślne zasady pitchu i system designu przenieś z niego 1:1, chyba że niżej napisano inaczej.

Zacznij w **trybie planowania**: zaproponuj architekturę, strukturę repo i kolejność etapów, poczekaj na moją akceptację, potem buduj etapami. Po każdym etapie aplikacja ma się kompilować i uruchamiać.

## Do czego służy

Narzędzie dla managera muzycznego (Repitch, polski hip-hop i pop) do pisania pitchy redakcyjnych do Spotify for Artists:
1. Użytkownik wrzuca plik audio (master), podaje artystę, tytuł, featy, datę premiery, kontekst kampanii, opcjonalnie tekst.
2. **Gemini odsłuchuje utwór** i zwraca strukturalną analizę (gatunek i podgatunki z uzasadnieniem, BPM, tonacja, energia, nastroje, instrumenty, produkcja, wokal, tematy tekstu, hook, struktura i momenty z timestampami, kąt redakcyjny).
3. Jeśli artysta nie ma jeszcze **pigułki**, Gemini z Google Search robi pogłębiony research (dossier ze źródłami), a z dossier powstaje pigułka — edytowalna notatka o artyście.
4. Generator pisze **pitch** (EN, max 500 znaków) wg edytowalnych zasad + wzorcowych pitchy, z pigułki, notatek managera, kontekstu kampanii i analizy. Dodatkowo propozycje pól formularza Spotify for Artists.
5. Użytkownik edytuje pitch, prosi o poprawki wg uwag, generuje nowe wersje (bez ponownego odsłuchu), wszystko z historią.

Zasada nadrzędna: **użytkownik ma kontrolę nad wiedzą o artyście.** Research nigdy nie nadpisuje pigułki — przychodzi jako propozycja do porównania (zastąp / dopisz / odrzuć). Notatki managera mają najwyższy priorytet w promptach i nic ich nie zmienia automatycznie.

## Stos technologiczny (zaproponuj zmiany w planie, jeśli masz mocne powody)

- **Tauri 2** (Rust) + **React + TypeScript + Vite** na froncie. Uzasadnienie: małe instalatory, natywne okno, wywołania API z backendu (bez problemów z CORS i sandboxem przeglądarki).
- **SQLite** jako lokalna baza (np. `sqlx` z migracjami albo `tauri-plugin-sql`). Plik bazy w katalogu danych aplikacji użytkownika.
- **Klucze API w systemowym sejfie** (Windows Credential Manager / macOS Keychain, crate `keyring`), nigdy w bazie ani w plikach konfiguracyjnych.
- Wszystkie wywołania Gemini i Anthropic **wyłącznie z warstwy Rust** (komendy Tauri), z możliwością anulowania i postępem przesyłanym eventami do UI.
- Dekodowanie audio do waveformu: w Rust (`symphonia`) lub Web Audio na froncie — wybierz, co działa dla MP3/WAV/FLAC/M4A/AIFF.
- Instalatory: `.msi`/NSIS dla Windows, `.dmg` dla macOS. **GitHub Actions** budujące oba przy tagu wersji. **Auto-update** (`tauri-plugin-updater`) z GitHub Releases.
- Lint + format (ESLint, Prettier, clippy, rustfmt), testy (Vitest na froncie, `cargo test` w Rust).

## Model danych (SQLite — doprecyzuj w planie)

- `artists` — id, name, slug (unikalny, bez polskich znaków), pill, manager_notes, research_hints, dossier, created_at, updated_at, researched_at
- `artist_sources` — artist_id, title, url, fetched_at
- `pill_proposals` — artist_id, pill, dossier, sources_json, created_at, status (pending / accepted / appended / rejected)
- `pill_history` — artist_id, pill, changed_at, change_type (manual / research_accept / append) — żeby dało się cofnąć zmianę pigułki
- `tracks` — id, artist_id, title, featuring, release_date, campaign_context, lyrics, audio_path (kopia lub ścieżka do pliku), audio_hash, duration_s, created_at
- `analyses` — track_id, model, json, created_at
- `pitches` — id, track_id, text, text_pl, form_json, angle, facts_used_json, engine, model, is_manual_edit, is_current, created_at
- `settings` — klucz/wartość (bez sekretów): wybrane modele, silnik pitchu, język, ścieżki
- `rules` — wersjonowane zasady pitchu i wzorcowe pitche (z historią zmian)

Migracje wersjonowane od pierwszego dnia. Baza ma działać offline; sieć jest potrzebna tylko do wywołań modeli.

## Integracje

**Gemini (wymagane)**
- Lista modeli pobierana z `GET /v1beta/models` (filtr: `generateContent`, rodzina Gemini, bez embed/image/tts/live). Domyślnie najnowszy „pro”. Osobny wybór modelu do odsłuchu i do researchu.
- Audio: pliki < ~14 MB inline (`inline_data`), większe przez **Files API** (resumable upload, polling do `ACTIVE`). Cache `file_uri` po hashu pliku, dopóki plik w Gemini nie wygaśnie.
- Research: `tools: [{ google_search: {} }]`, zbieranie `groundingMetadata.groundingChunks` jako źródeł i `webSearchQueries` do podglądu.
- Odsłuch: `responseMimeType: application/json`, walidacja wyniku schematem (np. zod / serde) z czytelnym błędem, jeśli model zwróci coś innego.

**Anthropic Claude (opcjonalny silnik pitchu)** — Messages API, model wybierany w ustawieniach (domyślnie `claude-opus-5-5`, alternatywa `claude-sonnet-5-5`).

**Pitch** — jeśli wynik > 500 znaków, jedno automatyczne skrócenie; licznik i miernik w UI zawsze liczą znaki tak jak Spotify (ze spacjami).

Obsługa błędów: limity zapytań (429) z jednym ponowieniem po odczekaniu, brak klucza, zły klucz, brak sieci, timeout — każdy z komunikatem po polsku, co zrobić. Żadnych cichych pętli ponawiania.

## Ekrany (port z prototypu + rozszerzenia)

1. **Analiza i pitch** — drop pliku, odtwarzacz z waveformem (klik = przewijanie), formularz, pipeline w 5 numerowanych krokach z postępem i przyciskiem Zatrzymaj, panel wyniku: pitch z miernikiem 500 znaków, kopiuj, zapisz edycję, nowa wersja, popraw wg uwag, dodaj do wzorców, tłumaczenie PL, pola Spotify for Artists, użyte fakty, pełna analiza, historia wersji.
2. **Artyści** — lista z wyszukiwaniem, edytor pigułki / notatek / wskazówek do researchu, przycisk researchu, porównanie propozycji side-by-side (najlepiej z podświetleniem różnic), źródła, dossier, historia pigułki z przywracaniem, lista pitchy artysty.
3. **Historia** — tabela pitchy z filtrem po artyście i wyszukiwaniem, sortowanie po dacie.
4. **Zasady** — edytor zasad i wzorcowych pitchy z historią wersji, przywróć domyślne, język pitchu.
5. **Ustawienia** — klucze (sejf systemowy), test połączenia, modele, silnik pitchu, folder danych, eksport / import, sprawdzanie aktualizacji, wersja aplikacji.

Dodatkowo:
- **Import z wersji HTML**: wczytanie pliku JSON eksportowanego przez prototyp (struktura: `{ artists: {id: {...}}, tracks: [...], rules, examples, lang }`) do SQLite.
- **Eksport / kopia zapasowa**: cała baza do jednego pliku (ZIP z bazą i opcjonalnie audio) i przywracanie z niego.
- **Kreator pierwszego uruchomienia**: link do aistudio.google.com/apikey, wklejenie klucza, test, wybór modelu. Bez tego reszta aplikacji jest zablokowana z jasnym komunikatem.

## Design

Przenieś system designu z prototypu: tokeny kolorów (ciemny grafit ze śliwkowym odcieniem, liliowy akcent, zielony sygnałowy, jasny wariant), fonty Unbounded (logotyp i nagłówki) / Manrope (tekst) / JetBrains Mono (dane), segmentowy miernik VU, znak z paskami waveformu, logotyp „Soundvice pitch**LAB**”. Fonty dołącz lokalnie do aplikacji (bez Google Fonts w runtime). Motyw: systemowy + przełącznik. Minimalny rozmiar okna ok. 1000×700, układ responsywny w oknie. Pełna obsługa klawiatury i widoczny fokus. Ikony SVG (np. Lucide), żadnych emoji. Ikona aplikacji wygenerowana ze znaku.

## Dystrybucja i udostępnianie

- Każdy użytkownik wpisuje **własny** klucz Gemini (i opcjonalnie Anthropic) — aplikacja nie zawiera żadnych kluczy w kodzie.
- README dla użytkownika końcowego po polsku: instalacja, skąd wziąć klucz, ostrzeżenie, że na darmowym planie Gemini Google może wykorzystywać przesłane dane (przy niewydanych masterach zalecany płatny plan).
- README techniczne: jak zbudować, jak wydać wersję, podpisywanie kodu.
- Podpisywanie: przygotuj konfigurację i sekrety w CI pod certyfikat Windows i Apple Developer ID (notaryzacja), ale niech build działa też bez nich (z ostrzeżeniem SmartScreen / Gatekeeper opisanym w README).

## Kryteria odbioru

- Instalator Windows i macOS budowany przez CI z taga `v*`; aplikacja instaluje się i uruchamia na czystym systemie.
- Pełny przepływ: plik 40 MB WAV → odsłuch → research nowego artysty → pigułka → pitch ≤ 500 znaków, z widocznym postępem i możliwością anulowania na każdym etapie.
- Ponowny research istniejącego artysty tworzy propozycję i nie zmienia pigułki, dopóki użytkownik jej nie przyjmie.
- Import JSON z wersji HTML przenosi artystów, pitche, zasady i wzorce.
- Klucze nie występują w bazie, logach ani eksportach.
- Testy jednostkowe: liczenie znaków, parsowanie JSON z modeli, slugi (polskie znaki), migracje, import.

## Sposób pracy

- Najpierw plan i struktura, potem etapy: (1) szkielet Tauri + baza + migracje, (2) ustawienia i sejf na klucze, (3) Gemini: odsłuch, (4) research i pigułki, (5) pitch i wersje, (6) historia, zasady, import/eksport, (7) design i dopracowanie, (8) CI, instalatory, auto-update.
- Commituj po każdym etapie z opisem.
- Gdy masz wątpliwość co do zachowania, sprawdź prototyp HTML. Gdy prototyp nie rozstrzyga, zapytaj mnie zamiast zgadywać.
