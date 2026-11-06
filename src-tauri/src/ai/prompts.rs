//! Prompty do modeli — port 1:1 z prototypu (`legacy/soundvice-pitchlab.html`).

pub struct TrackInfo<'a> {
    pub title: &'a str,
    pub artist: &'a str,
    pub featuring: &'a str,
    pub lyrics: &'a str,
}

/// `analysisPrompt` z prototypu (l. 505).
pub fn analysis(t: &TrackInfo) -> String {
    let title = if t.title.trim().is_empty() {
        "bez tytułu"
    } else {
        t.title
    };
    let feat = if t.featuring.trim().is_empty() {
        String::new()
    } else {
        format!(" (feat./prod.: {})", t.featuring)
    };
    let lyrics = if t.lyrics.trim().is_empty() {
        String::new()
    } else {
        format!("Tekst / opis od managera:\n{}\n", t.lyrics)
    };
    format!(
        r#"Jesteś doświadczonym A&R i inżynierem dźwięku. Uważnie odsłuchaj CAŁY załączony utwór i przeanalizuj go.
Utwór: "{title}" — {artist}{feat}.
{lyrics}
Zasady: opisuj tylko to, co faktycznie słychać. Gatunki podawaj angielskimi nazwami używanymi w branży, możliwie precyzyjnie (podgatunek). Jeśli numer jest hybrydą, nazwij składniki. Timestampy w formacie m:ss. Opisy po polsku.
Zwróć wyłącznie JSON:
{{
 "primary_genre": "string",
 "subgenres": ["string"],
 "genre_rationale": "1-2 zdania: po czym to słychać",
 "bpm": number | null,
 "key": "string | null",
 "energy": 1-10,
 "moods": ["angielskie przymiotniki"],
 "instruments": ["string"],
 "production_notes": "2-3 zdania o produkcji i miksie",
 "vocals": {{"type":"rap/śpiew/melodyjny rap…","language":"string","delivery":"opis flow / barwy","features":"string | null"}},
 "lyrics_themes": ["string"],
 "lyrics_summary": "2-3 zdania o czym jest tekst",
 "hook": "opis refrenu/haczyka",
 "structure": [{{"t":"0:00","section":"intro"}}],
 "standout_moments": [{{"t":"0:00","desc":"string"}}],
 "sound_alikes": ["artysta — utwór (dla orientacji, nie do pitchu)"],
 "explicit_content": true|false,
 "editorial_angle": "1 zdanie: co w tym numerze jest najciekawsze z perspektywy redaktora playlist",
 "opening_lines": "dosłowny zapis pierwszych 1-2 linijek tekstu dokładnie tak, jak je słyszysz (pusty napis, jeśli nie ma wokalu lub nie rozumiesz słów — nie zgaduj)",
 "first_vocal_at": "m:ss — kiedy wchodzi pierwszy wokal"
}}"#,
        artist = t.artist,
    )
}

/// Kontekst bieżącego singla dla researchu (opcjonalny).
pub struct ResearchCtx<'a> {
    pub title: &'a str,
    pub genre: &'a str,
}

/// `researchPrompt` z prototypu (l. 532).
pub fn research(name: &str, hints: &str, ctx: Option<&ResearchCtx>) -> String {
    let hints = if hints.trim().is_empty() {
        String::new()
    } else {
        format!("Wskazówki identyfikacyjne od managera: {hints}\n")
    };
    let ctx = match ctx {
        Some(c) => {
            let genre = if c.genre.is_empty() {
                String::new()
            } else {
                format!(", gatunek: {}", c.genre)
            };
            format!("Kontekst: aktualny singiel \"{}\"{genre}.\n", c.title)
        }
        None => String::new(),
    };
    format!(
        r#"Przeprowadź pogłębiony research w internecie o polskim artyście muzycznym "{name}".
{hints}{ctx}
Uwaga na artystów o podobnych pseudonimach — na początku napisz, po czym zidentyfikowałeś właściwą osobę.
Szukaj i opisz z datami:
- kim jest: prawdziwe imię (jeśli publiczne), miasto, rok debiutu, wytwórnia/management, kolektyw
- dyskografia: albumy, EP-ki, najważniejsze single (z rokiem)
- liczby: miesięczni słuchacze Spotify, najpopularniejsze utwory i ich odsłuchy, YouTube, pozycje OLiS / listy
- certyfikaty ZPAV (złoto/platyna/diament — który utwór/album, kiedy)
- nagrody i nominacje (Fryderyki, Popkillery itp.)
- współprace i featuringi z rozpoznawalnymi artystami, producenci
- koncerty, trasy, festiwale
- media: ważne wywiady, recenzje, wzmianki
- social media: zasięgi (IG, TikTok), wiralowe momenty
- unikalna historia / narracja artysty, styl, tematyka tekstów
Każdą liczbę opatrz źródłem i datą. Fakty, których nie da się potwierdzić w co najmniej jednym wiarygodnym źródle, oznacz [?]. Nie zgaduj. Pisz po polsku, w punktach, z nagłówkami."#
    )
}

/// `pillPrompt` z prototypu (l. 549).
pub fn pill(name: &str, dossier: &str, old_pill: &str) -> String {
    let old = if old_pill.trim().is_empty() {
        String::new()
    } else {
        format!(
            "Obecna pigułka (zachowaj z niej to, co nadal aktualne, popraw to, co dossier aktualizuje):\n{old_pill}\n"
        )
    };
    format!(
        r#"Na podstawie dossier z researchu przygotuj PIGUŁKĘ o artyście "{name}" — zwięzłą notatkę roboczą dla managera piszącego pitche do Spotify. Po polsku, maks. ~1800 znaków, tylko fakty z dossier. Zachowaj oznaczenia [?] przy niepewnych faktach.
{old}
Użyj dokładnie tych sekcji:
KIM JEST:
BRZMIENIE I TEMATY:
KAMIENIE MILOWE (liczby, certyfikaty, nagrody — z datą):
WYDAWNICTWA:
WSPÓŁPRACE:
NARRACJA DO PITCHU (1-2 najmocniejsze kąty):
NIEPEWNE / NIE UŻYWAĆ:

DOSSIER:
{dossier}

Zwróć sam tekst pigułki, bez wstępu."#
    )
}

/// Dane do promptu pitchu (`pitchPrompt` z prototypu, l. 566).
pub struct PitchInput<'a> {
    pub rules: &'a str,
    pub examples: &'a str,
    pub polish: bool,
    pub title: &'a str,
    pub artist: &'a str,
    pub featuring: &'a str,
    pub release_date: &'a str,
    pub manager_notes: &'a str,
    pub campaign_context: &'a str,
    /// Tekst lub opis tekstu od managera (o czym jest utwór).
    pub lyrics: &'a str,
    pub pill: &'a str,
    pub analysis_json: &'a str,
    /// Dodatkowy blok: prośba o nową wersję albo poprawki wg uwag.
    pub extra: &'a str,
}

fn or_dash(s: &str) -> &str {
    if s.trim().is_empty() { "—" } else { s.trim() }
}

pub fn pitch(i: &PitchInput) -> String {
    let lang = if i.polish { "Polish" } else { "English" };
    let examples = if i.examples.trim().is_empty() {
        String::new()
    } else {
        format!(
            "=== APPROVED EXAMPLE PITCHES (match tone and density, never copy facts) ===\n{}\n",
            i.examples
        )
    };
    let feat = if i.featuring.trim().is_empty() {
        String::new()
    } else {
        format!("\nFeaturing / producer: {}", i.featuring)
    };
    let lyrics_block = if i.lyrics.trim().is_empty() {
        String::new()
    } else {
        format!(
            "=== LYRICS / DESCRIPTION FROM MANAGER (what the song is about) ===\n{}\n\n",
            i.lyrics.trim()
        )
    };
    let pill = if i.pill.trim().is_empty() {
        "— (no pill: do not state any artist facts beyond the name)"
    } else {
        i.pill.trim()
    };
    format!(
        r#"You are a senior music marketer at Repitch (Polish label & management) writing a Spotify for Artists editorial pitch.

=== HOUSE RULES (follow strictly; written by the manager in Polish) ===
{rules}

{examples}
=== TRACK ===
Title: {title}
Artist: {artist}{feat}
Release date: {date}

=== MANAGER NOTES (highest priority) ===
{notes}

=== CAMPAIGN CONTEXT ===
{ctx}

{lyrics}=== ARTIST PILL (verified facts; ignore anything marked [?]) ===
{pill}

=== AUDIO ANALYSIS (what is actually audible) ===
{analysis}

{extra}
Write the pitch in {lang}. HARD LIMIT: the "pitch" string must be at most 500 characters including spaces; aim for 380-480.
Also propose Spotify for Artists form fields based on the audio (use Spotify's usual wording; max 3 genres, max 3 moods).
Return ONLY JSON:
{{"pitch":"string","pitch_pl":"Polish translation for the manager (if pitch is already Polish, repeat it)","form":{{"genres":["max 3"],"moods":["max 3"],"styles":["e.g. Pop, Rap, Electronic subtypes"],"instruments":["string"],"culture":"string or empty","song_type":"e.g. Original / Remix / Acoustic","vocal_language":"string"}},"angle":"one sentence: which angle you chose and why","facts_used":["each artist/campaign fact used and its source: pill / notes / context / analysis"]}}"#,
        rules = i.rules,
        title = or_dash(i.title),
        artist = i.artist,
        date = or_dash(i.release_date),
        notes = or_dash(i.manager_notes),
        ctx = or_dash(i.campaign_context),
        lyrics = lyrics_block,
        analysis = i.analysis_json,
        extra = i.extra,
    )
}

/// Prośba o wyraźnie inną wersję niż poprzednia.
pub fn new_version_extra(previous: &str) -> String {
    format!(
        "=== NEW VERSION ===\nWrite a clearly different take than this previous version (different opening and angle):\n{previous}\n"
    )
}

/// Poprawki wg uwag managera.
pub fn revision_extra(current: &str, feedback: &str) -> String {
    format!(
        "=== REVISION REQUEST ===\nCurrent pitch:\n{current}\nManager feedback (apply it, keep everything else that works):\n{feedback}\n"
    )
}

/// Jednorazowe skrócenie pitchu powyżej limitu 500 znaków.
pub fn shorten(pitch: &str) -> String {
    format!(
        "Shorten this Spotify editorial pitch to at most 480 characters including spaces. Keep the genre framing, the strongest fact and the promo plan. Do not add anything new. Return ONLY JSON {{\"pitch\":\"...\",\"pitch_pl\":\"...\"}}\n\n{pitch}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input<'a>() -> PitchInput<'a> {
        PitchInput {
            rules: "ZASADY-TEKST",
            examples: "",
            polish: false,
            title: "RETRO",
            artist: "Mata",
            featuring: "",
            release_date: "",
            manager_notes: "",
            campaign_context: "TikTok",
            lyrics: "",
            pill: "",
            analysis_json: "{\"primary_genre\":\"rap\"}",
            extra: "",
        }
    }

    #[test]
    fn pitch_prompt_sections_and_defaults() {
        let p = pitch(&input());
        assert!(p.contains(
            "=== HOUSE RULES (follow strictly; written by the manager in Polish) ===\nZASADY-TEKST"
        ));
        assert!(!p.contains("APPROVED EXAMPLE PITCHES"));
        assert!(!p.contains("LYRICS / DESCRIPTION"));
        assert!(p.contains("Title: RETRO\nArtist: Mata\nRelease date: —"));
        assert!(p.contains("=== MANAGER NOTES (highest priority) ===\n—"));
        assert!(p.contains("— (no pill: do not state any artist facts beyond the name)"));
        assert!(p.contains("Write the pitch in English. HARD LIMIT"));
        assert!(p.contains(r#"{"pitch":"string","pitch_pl":"#));
        assert!(p.contains(r#""facts_used":["each artist/campaign fact used"#));
    }

    #[test]
    fn pitch_prompt_with_everything() {
        let mut i = input();
        i.examples = "PRZYKŁAD";
        i.featuring = "Białas";
        i.polish = true;
        i.pill = "PIGUŁKA";
        i.manager_notes = "NOTATKI";
        i.extra = "EXTRA-BLOK";
        i.lyrics = "o tęsknocie za latem";
        let p = pitch(&i);
        assert!(p.contains(
            "=== APPROVED EXAMPLE PITCHES (match tone and density, never copy facts) ===\nPRZYKŁAD"
        ));
        assert!(p.contains("Artist: Mata\nFeaturing / producer: Białas"));
        assert!(
            p.contains("=== ARTIST PILL (verified facts; ignore anything marked [?]) ===\nPIGUŁKA")
        );
        assert!(p.contains("EXTRA-BLOK\nWrite the pitch in Polish."));
        assert!(p.contains(
            "=== LYRICS / DESCRIPTION FROM MANAGER (what the song is about) ===\no tęsknocie za latem\n\n=== ARTIST PILL"
        ));
    }

    #[test]
    fn extras_and_shorten() {
        assert!(new_version_extra("OLD").contains("different opening and angle):\nOLD\n"));
        let r = revision_extra("CUR", "krócej");
        assert!(r.contains("Current pitch:\nCUR\nManager feedback (apply it, keep everything else that works):\nkrócej"));
        assert!(shorten("X").ends_with("\n\nX"));
        assert!(shorten("X").contains("at most 480 characters"));
    }

    #[test]
    fn research_prompt_includes_hints_and_context() {
        let p = research(
            "Mata",
            "raper z Warszawy",
            Some(&ResearchCtx {
                title: "RETRO",
                genre: "rap",
            }),
        );
        assert!(p.contains(r#"o polskim artyście muzycznym "Mata"."#));
        assert!(p.contains("Wskazówki identyfikacyjne od managera: raper z Warszawy\n"));
        assert!(p.contains(r#"Kontekst: aktualny singiel "RETRO", gatunek: rap."#));
        assert!(p.contains("oznacz [?]"));
        let bare = research("X", "  ", None);
        assert!(!bare.contains("Wskazówki"));
        assert!(!bare.contains("Kontekst:"));
    }

    #[test]
    fn pill_prompt_embeds_dossier_and_old_pill() {
        let p = pill("Mata", "DOSSIER-TEKST", "STARA PIGUŁKA");
        assert!(p.contains("DOSSIER:\nDOSSIER-TEKST"));
        assert!(p.contains("Obecna pigułka (zachowaj z niej to, co nadal aktualne, popraw to, co dossier aktualizuje):\nSTARA PIGUŁKA"));
        assert!(p.contains("NIEPEWNE / NIE UŻYWAĆ:"));
        assert!(!pill("M", "d", "").contains("Obecna pigułka"));
    }

    #[test]
    fn analysis_prompt_includes_track_data_and_schema() {
        let p = analysis(&TrackInfo {
            title: "RETRO",
            artist: "Mata",
            featuring: "Białas",
            lyrics: "tekst",
        });
        assert!(p.contains(r#"Utwór: "RETRO" — Mata (feat./prod.: Białas)."#));
        assert!(p.contains("Tekst / opis od managera:\ntekst"));
        assert!(p.contains(r#""primary_genre": "string""#));
        assert!(p.contains(r#""structure": [{"t":"0:00","section":"intro"}]"#));
        assert!(p.contains("\"opening_lines\":"));
        assert!(p.contains("\"first_vocal_at\":"));
    }

    #[test]
    fn untitled_and_no_extras() {
        let p = analysis(&TrackInfo {
            title: " ",
            artist: "X",
            featuring: "",
            lyrics: "",
        });
        assert!(p.contains(r#"Utwór: "bez tytułu" — X."#));
        assert!(!p.contains("Tekst / opis"));
    }
}
