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
 "editorial_angle": "1 zdanie: co w tym numerze jest najciekawsze z perspektywy redaktora playlist"
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

#[cfg(test)]
mod tests {
    use super::*;

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
