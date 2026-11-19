//! Accent- and case-insensitive text folding, the `normalize('NFD').replace(combining, '')
//! .toLowerCase()` of the React search and palette code. Only letters that decompose under NFD
//! lose their accent (`ø`, `ß`, `æ` stay as they are, like in JavaScript).

/// One char folded: accent removed, lower-cased.
pub fn fold_char(c: char) -> char {
    let base = match c {
        'à'..='å' | 'À'..='Å' | 'ā' | 'Ā' | 'ă' | 'Ă' | 'ą' | 'Ą' => 'a',
        'ç' | 'Ç' | 'ć' | 'Ć' | 'ĉ' | 'Ĉ' | 'ċ' | 'Ċ' | 'č' | 'Č' => 'c',
        'ď' | 'Ď' => 'd',
        'è'..='ë' | 'È'..='Ë' | 'ē' | 'Ē' | 'ĕ' | 'Ĕ' | 'ė' | 'Ė' | 'ę' | 'Ę' | 'ě' | 'Ě' => {
            'e'
        }
        'ĝ' | 'Ĝ' | 'ğ' | 'Ğ' | 'ġ' | 'Ġ' | 'ģ' | 'Ģ' => 'g',
        'ĥ' | 'Ĥ' => 'h',
        'ì'..='ï' | 'Ì'..='Ï' | 'ĩ' | 'Ĩ' | 'ī' | 'Ī' | 'ĭ' | 'Ĭ' | 'į' | 'Į' | 'İ' => {
            'i'
        }
        'ĵ' | 'Ĵ' => 'j',
        'ķ' | 'Ķ' => 'k',
        'ĺ' | 'Ĺ' | 'ļ' | 'Ļ' | 'ľ' | 'Ľ' => 'l',
        'ñ' | 'Ñ' | 'ń' | 'Ń' | 'ņ' | 'Ņ' | 'ň' | 'Ň' => 'n',
        'ò'..='ö' | 'Ò'..='Ö' | 'ō' | 'Ō' | 'ŏ' | 'Ŏ' | 'ő' | 'Ő' => 'o',
        'ŕ' | 'Ŕ' | 'ŗ' | 'Ŗ' | 'ř' | 'Ř' => 'r',
        'ś' | 'Ś' | 'ŝ' | 'Ŝ' | 'ş' | 'Ş' | 'š' | 'Š' => 's',
        'ţ' | 'Ţ' | 'ť' | 'Ť' => 't',
        'ù'..='ü'
        | 'Ù'..='Ü'
        | 'ũ'
        | 'Ũ'
        | 'ū'
        | 'Ū'
        | 'ŭ'
        | 'Ŭ'
        | 'ů'
        | 'Ů'
        | 'ű'
        | 'Ű'
        | 'ų'
        | 'Ų' => 'u',
        'ŵ' | 'Ŵ' => 'w',
        'ý' | 'ÿ' | 'Ý' | 'ŷ' | 'Ŷ' | 'Ÿ' => 'y',
        'ź' | 'Ź' | 'ż' | 'Ż' | 'ž' | 'Ž' => 'z',
        c => c,
    };
    base.to_lowercase().next().unwrap_or(base)
}

/// `normalize`: lower-case without accents ("Éléphant" -> "elephant"), combining marks dropped.
pub fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| !('\u{300}'..='\u{36f}').contains(c))
        .map(fold_char)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folds_accents_and_case() {
        assert_eq!(normalize("Éléphant"), "elephant");
        assert_eq!(normalize("Çà et là"), "ca et la");
        assert_eq!(normalize("Łódź"), "łodz");
        assert_eq!(normalize("e\u{301}te\u{301}"), "ete");
    }
}
