//! Who the church is: its name, website, where its services are listed and its pictures.
//! Everything that makes the program "HHG Tholen" is here, set in the console (Instellingen,
//! Onze kerk), so another church can use the program by filling in its own. The defaults are
//! those of HHG Tholen, so an existing installation stays as it was.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Kerk {
    /// "Hersteld Hervormde Gemeente Tholen": on the welcome and end boards, under the YouTube
    /// broadcast and on the watch page.
    pub naam: String,
    /// "www.hhgtholen.nl", as people read it (on the end screen).
    pub website: String,
    /// The page with the services and their liturgy; empty = not read, the operator types it.
    pub liturgie_url: String,
    /// The church's number on kerktijden.nl, for the predikant and the next service; 0 = off.
    pub kerktijden_id: u32,
    /// Pictures from the church's own (Instellingen, afbeeldingen) or the built-in ones: the
    /// start screen, the end screen and the logo (white on transparent).
    pub foto_start: String,
    pub foto_einde: String,
    pub logo: String,
    /// A dash between verses means "to" ("Ps 27: 3-7" is 3 to 7), as most churches write
    /// it. Off (HHG Tholen): "and", 3 and 7.
    pub streepje_tot_en_met: bool,
}

impl Default for Kerk {
    fn default() -> Kerk {
        Kerk {
            naam: "Hersteld Hervormde Gemeente Tholen".into(),
            website: "www.hhgtholen.nl".into(),
            liturgie_url: "https://www.hhgtholen.nl/kerkdiensten".into(),
            kerktijden_id: 57,
            foto_start: "buiten.jpg".into(),
            foto_einde: "interieur.jpg".into(),
            logo: "logo-wit.png".into(),
            streepje_tot_en_met: false,
        }
    }
}

impl Kerk {
    /// The website without "https://" and a closing "/", as it reads on a screen.
    pub fn website_text(&self) -> String {
        let w = self.website.trim();
        let w = w.strip_prefix("https://").or_else(|| w.strip_prefix("http://")).unwrap_or(w);
        w.trim_end_matches('/').to_string()
    }

    /// "Hersteld Hervormde Gemeente Tholen — www.hhgtholen.nl", or the part that is filled in.
    pub fn signature(&self) -> String {
        [self.naam.trim().to_string(), self.website_text()].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(" — ")
    }

    /// "Voor meer informatie over onze gemeente kijk op www.hhgtholen.nl" on the end screen;
    /// None without a website.
    pub fn more_info(&self) -> Option<String> {
        let w = self.website_text();
        (!w.is_empty()).then(|| format!("Voor meer informatie over onze gemeente kijk op {w}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn texts_from_what_is_filled_in() {
        let k = Kerk::default();
        assert_eq!(k.signature(), "Hersteld Hervormde Gemeente Tholen — www.hhgtholen.nl");
        assert_eq!(k.more_info().as_deref(), Some("Voor meer informatie over onze gemeente kijk op www.hhgtholen.nl"));
        let other = Kerk { naam: "Protestantse Gemeente Ergens".into(), website: "https://www.pgergens.nl/".into(), ..Default::default() };
        assert_eq!(other.signature(), "Protestantse Gemeente Ergens — www.pgergens.nl");
        let bare = Kerk { naam: String::new(), website: " ".into(), ..Default::default() };
        assert_eq!(bare.signature(), "");
        assert_eq!(bare.more_info(), None);
    }

    #[test]
    fn an_old_settings_file_gets_the_defaults() {
        let k: Kerk = serde_json::from_str(r#"{"naam":"Andere kerk"}"#).unwrap();
        assert_eq!(k.naam, "Andere kerk");
        assert_eq!(k.kerktijden_id, 57);
    }
}
