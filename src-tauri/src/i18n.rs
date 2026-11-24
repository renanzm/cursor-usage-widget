//! Idioma dos textos gerados no Rust (bandeja, alertas, mensagens de erro).
//! A interface (ui/i18n.js) usa o mesmo idioma resolvido, lido por `get_lang`.
use crate::settings::Language;
use std::sync::atomic::{AtomicBool, Ordering};

static ENGLISH: AtomicBool = AtomicBool::new(true);

/// `Auto` segue o idioma do Windows: português se começar com "pt", senão inglês.
pub fn resolve(lang: Language) -> Language {
    match lang {
        Language::Auto => {
            let sys = sys_locale::get_locale().unwrap_or_default().to_ascii_lowercase();
            if sys.starts_with("pt") {
                Language::Pt
            } else {
                Language::En
            }
        }
        l => l,
    }
}

pub fn set(lang: Language) {
    ENGLISH.store(resolve(lang) != Language::Pt, Ordering::Relaxed);
}

pub fn is_en() -> bool {
    ENGLISH.load(Ordering::Relaxed)
}

/// Código usado pela interface: "en" ou "pt".
pub fn code() -> &'static str {
    if is_en() {
        "en"
    } else {
        "pt"
    }
}

/// Escolhe o texto do idioma atual.
pub fn tr<'a>(pt: &'a str, en: &'a str) -> &'a str {
    if is_en() {
        en
    } else {
        pt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_language_wins() {
        assert_eq!(resolve(Language::Pt), Language::Pt);
        assert_eq!(resolve(Language::En), Language::En);
        assert_ne!(resolve(Language::Auto), Language::Auto);
    }
}
