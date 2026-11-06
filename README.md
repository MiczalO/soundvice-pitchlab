# Soundvice pitchLAB

Aplikacja desktopowa (Windows i macOS) do pisania pitchy redakcyjnych do **Spotify for Artists**.
Wgrywasz master, a program:

1. **odsłuchuje utwór** (Gemini) i opisuje gatunek, brzmienie, tekst, strukturę,
2. prowadzi **pigułkę artysty** (Twoją notatkę o artyście; research tylko proponuje zmiany),
3. pisze **pitch (max 500 znaków)** według Twoich zasad i wzorcowych pitchy,
4. pozwala go poprawiać, tworzyć nowe wersje bez ponownego odsłuchu i wracać do starszych.

Wszystkie dane są **lokalnie na Twoim komputerze**. Klucze API trzyma sejf systemowy.

## Instalacja

1. Wejdź w zakładkę **Releases** tego repozytorium i pobierz instalator:
   - **Windows:** plik `.exe` (zalecany, instaluje się bez uprawnień administratora) albo `.msi`,
   - **macOS:** plik `.dmg` (Apple Silicon: `aarch64`, starsze Maki z Intelem: `x64`).
2. Uruchom instalator i postępuj według instrukcji.
3. Przy pierwszym uruchomieniu kreator poprosi o klucz API (patrz niżej).

### Ostrzeżenia przy instalacji (gdy instalator nie jest podpisany)

Jeśli wydanie nie zostało podpisane certyfikatem, system pokaże ostrzeżenie. To normalne i nie oznacza,
że plik jest groźny, ale instaluj tylko instalatory pobrane z oficjalnych Releases tego repozytorium.

- **Windows (SmartScreen):** „System Windows ochronił ten komputer” → kliknij **Więcej informacji** → **Uruchom mimo to**.
- **macOS (Gatekeeper):** „nie można otworzyć, ponieważ nie można zweryfikować dewelopera” → w Finderze kliknij aplikację
  prawym przyciskiem → **Otwórz** → **Otwórz**. Jeśli to nie pomoże: *Ustawienia systemowe → Prywatność i ochrona →
  Zabezpieczenia → Otwórz mimo to*.

## Klucz API Gemini (wymagany)

Każdy użytkownik używa **własnego** klucza. Aplikacja nie zawiera żadnych kluczy.

1. Wejdź na <https://aistudio.google.com/apikey> i utwórz klucz (**Get API key**).
2. Wklej go w kreatorze pierwszego uruchomienia (albo w **Ustawieniach**). Klucz trafia do sejfu systemowego
   (Menedżer poświadczeń Windows / Pęk kluczy macOS), nigdy do bazy ani do plików.
3. Aplikacja sprawdzi, które modele działają dla Twojego klucza, i sama wybierze działający.

> **Ważne o prywatności i limitach.** Na **darmowym planie** Gemini Google może wykorzystywać przesłane dane.
> Przy **niewydanych masterach** zalecany jest **płatny plan**. Darmowy plan ma też niskie dzienne limity, a **wyszukiwarka
> Google (research artysty) zwykle wymaga płatnego planu**. Bez niej pigułkę wpisujesz ręcznie albo wklejasz własny research
> w karcie Artyści; odsłuch i pitch działają bez wyszukiwarki.

Opcjonalnie: klucz **Anthropic (Claude)**, jeśli chcesz, żeby to Claude pisał pitche (Ustawienia → Silnik pitchu).

## Jak pracować

- **Analiza i pitch:** upuść plik audio (MP3, WAV, FLAC, M4A, OGG, AIFF), podaj artystę, tytuł, feat./producenta
  (np. `feat. Gedz; prod. Jakub Gendźwiłł`), kontekst kampanii i ewentualnie tekst. Kliknij **Analizuj**.
- **Dowód odsłuchu:** w analizie zobaczysz dosłowny początek tekstu, który model usłyszał, oraz ile dźwięku przetworzył.
- **Artyści:** pigułka, notatki managera (najwyższy priorytet), wskazówki do researchu, propozycje z różnicami
  (zastąp / dopisz / odrzuć) i historia pigułki z przywracaniem.
- **Zasady:** zasady pitchu i wzorcowe pitche. Każdy zapis to nowa wersja, którą można przywrócić.
- **Historia:** wszystkie utwory i pitche z filtrem po artyście i wyszukiwaniem.
- **Ustawienia:** klucze, modele, motyw, kopia bazy, import, aktualizacje.

Gdy Gemini jest przeciążony (błąd 503), aplikacja **sama ponawia próby** i przełącza się na inne działające modele.
Postęp widać w kroku, a przycisk **Zatrzymaj** przerywa pracę.

## Kopia zapasowa, przeniesienie na inny komputer, import

W **Ustawieniach → Kopia bazy i dane**:

- **Eksportuj kopię** zapisuje jeden plik ZIP (artyści, pigułki, pitche, zasady; opcjonalnie pliki audio).
  **Kopia nie zawiera kluczy API** — po przeniesieniu wpisz je od nowa.
- **Przywróć z kopii** podmienia dane po restarcie aplikacji. Poprzednia baza zostaje w folderze danych (`before_restore`).
- **Import z wersji HTML** wczytuje plik JSON z prototypu. Niczego nie nadpisuje: inna pigułka istniejącego artysty
  trafia jako propozycja, a ponowny import nie dubluje utworów.
- **Pokaż folder danych** otwiera katalog z bazą (`pitchlab.sqlite3`) i skopiowanymi plikami audio.

## Aktualizacje

Aplikacja sprawdza aktualizacje po starcie (jeśli jest internet) i w **Ustawienia → Aktualizacje**. Aktualizacje są
podpisane; instalacja uruchamia aplikację ponownie.

## Gdy coś nie działa

| Objaw | Co zrobić |
| --- | --- |
| „Model jest chwilowo przeciążony” | To po stronie Google. Aplikacja próbuje do skutku; możesz zatrzymać i wybrać inny model w Ustawieniach. |
| „Brak przydziału w Twoim planie” (429) | Model Pro lub wyszukiwarka Google nie są w Twoim planie. Wybierz model flash albo włącz płatny plan. |
| „Wyczerpany dzienny limit” | Darmowy limit dzienny się skończył. Poczekaj do następnego dnia lub włącz płatny plan. |
| Klucz „odrzucony” | Wklej klucz ponownie w całości (Ustawienia). |
| Research artysty nie działa | Wyszukiwarka Google wymaga płatnego planu. Wpisz pigułkę ręcznie albo użyj „Wklej własny research”. |

## Dla programistów

Budowanie, wydawanie wersji, podpisywanie i aktualizacje: [docs/BUILDING.md](docs/BUILDING.md).
