/// Slug artysty bez polskich znaków. Port `slug()` z prototypu:
/// małe litery, ł→l, usunięcie znaków diakrytycznych, nie-alfanumeryczne → "-".
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    let mut last_dash = true;
    for ch in s.to_lowercase().chars() {
        let base = match ch {
            'ł' => 'l',
            'ą' => 'a',
            'ć' => 'c',
            'ę' => 'e',
            'ń' => 'n',
            'ó' => 'o',
            'ś' => 's',
            'ź' | 'ż' => 'z',
            c => c,
        };
        let base = fold_latin(base);
        if base.is_ascii_alphanumeric() {
            out.push(base);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    let t = out.trim_matches('-');
    if t.is_empty() {
        "artysta".to_string()
    } else {
        t.to_string()
    }
}

/// Podstawowe łacińskie litery z akcentami (á, é, ü, ñ …) → ASCII.
fn fold_latin(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' => 'a',
        'é' | 'è' | 'ê' | 'ë' => 'e',
        'í' | 'ì' | 'î' | 'ï' => 'i',
        'ò' | 'ô' | 'ö' | 'õ' | 'ø' => 'o',
        'ú' | 'ù' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        'ý' | 'ÿ' => 'y',
        c => c,
    }
}

/// Liczenie znaków pitchu jak Spotify: ze spacjami, w punktach kodowych.
pub fn char_count(s: &str) -> usize {
    s.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn polish_diacritics() {
        assert_eq!(slugify("Żółć Gęślą Jaźń"), "zolc-gesla-jazn");
        assert_eq!(slugify("Łąki ćma ńó"), "laki-cma-no");
    }

    #[test]
    fn punctuation_and_edges() {
        assert_eq!(slugify("  Taco Hemingway! "), "taco-hemingway");
        assert_eq!(slugify("O.S.T.R."), "o-s-t-r");
        assert_eq!(slugify("???"), "artysta");
        assert_eq!(slugify(""), "artysta");
    }

    #[test]
    fn counts_chars_not_bytes() {
        assert_eq!(char_count("zażółć"), 6);
        assert_eq!(char_count("a b"), 3);
    }
}
