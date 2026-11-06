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

#[cfg(test)]
mod tests {
    use super::*;

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
