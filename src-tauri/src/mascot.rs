//! Which mascot the widget shows. The widget draws them (ui/mascot.js); the
//! app only keeps the choice, offers it in the tray menu and saves it.

/// The mascots one can choose. The ids are the ones `ui/mascots.js` knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mascot {
    /// The one shown when nothing has been chosen.
    #[default]
    Chiikawa,
    Hachiware,
    Usagi,
    /// 칩이, the mascot drawn for this app.
    Chip,
}

impl Mascot {
    /// In the order the tray menu lists them.
    pub const ALL: [Mascot; 4] = [
        Mascot::Chiikawa,
        Mascot::Hachiware,
        Mascot::Usagi,
        Mascot::Chip,
    ];

    /// The name used in the saved choice and by the widget.
    pub fn id(self) -> &'static str {
        match self {
            Mascot::Chiikawa => "chiikawa",
            Mascot::Hachiware => "hachiware",
            Mascot::Usagi => "usagi",
            Mascot::Chip => "chip",
        }
    }

    /// The name in the tray menu.
    pub fn label(self) -> &'static str {
        match self {
            Mascot::Chiikawa => "치이카와",
            Mascot::Hachiware => "하치와레",
            Mascot::Usagi => "우사기",
            Mascot::Chip => "칩이",
        }
    }

    pub fn from_id(id: &str) -> Option<Mascot> {
        Mascot::ALL.into_iter().find(|m| m.id() == id)
    }

    /// The choice in the saved file's text. No file, or text that names no
    /// mascot, gives the default one.
    pub fn from_saved(text: Option<&str>) -> Mascot {
        text.and_then(|t| Mascot::from_id(t.trim()))
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_saved_means_chiikawa() {
        assert_eq!(Mascot::from_saved(None), Mascot::Chiikawa);
        assert_eq!(Mascot::default(), Mascot::Chiikawa);
    }

    #[test]
    fn saved_choice_round_trips() {
        for m in Mascot::ALL {
            assert_eq!(Mascot::from_saved(Some(m.id())), m);
        }
        assert_eq!(Mascot::from_saved(Some("usagi\n")), Mascot::Usagi);
    }

    #[test]
    fn odd_saved_text_means_the_default() {
        assert_eq!(Mascot::from_saved(Some("")), Mascot::Chiikawa);
        assert_eq!(Mascot::from_saved(Some("Usagi")), Mascot::Chiikawa);
        assert_eq!(Mascot::from_saved(Some("1")), Mascot::Chiikawa);
    }

    #[test]
    fn unknown_id_is_no_mascot() {
        assert_eq!(Mascot::from_id("nobody"), None);
        assert_eq!(Mascot::from_id("chip"), Some(Mascot::Chip));
    }

    #[test]
    fn menu_lists_the_three_pictures_then_chip() {
        let labels: Vec<_> = Mascot::ALL.iter().map(|m| m.label()).collect();
        assert_eq!(labels, ["치이카와", "하치와레", "우사기", "칩이"]);
    }

    /// The widget has its own list of the same ids and the same default.
    #[test]
    fn ids_match_the_widgets_list() {
        let js = include_str!("../../ui/mascots.js");
        let ids: Vec<_> = Mascot::ALL
            .iter()
            .map(|m| format!("'{}'", m.id()))
            .collect();
        assert!(js.contains(&format!("MASCOTS = [{}]", ids.join(", "))));
        assert!(js.contains(&format!("DEFAULT_MASCOT = '{}'", Mascot::default().id())));
    }
}
