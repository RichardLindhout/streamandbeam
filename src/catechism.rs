//! The Heidelberg Catechism (text of the Synod of Dordrecht, from the Gihonbron edition;
//! see assets/data/BRON.txt).

use crate::show::Item;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Debug, Clone, Deserialize)]
pub struct Qa {
    pub nummer: u32,
    pub zondag: u32,
    pub vraag: String,
    pub antwoord: String,
}

static ALL: LazyLock<Vec<Qa>> =
    LazyLock::new(|| serde_json::from_str(include_str!("../assets/data/catechismus.json")).expect("catechismus.json"));

/// The questions of a catechism item: its question numbers, or all of its Lord's Day.
pub fn for_item(item: &Item) -> Vec<Qa> {
    ALL.iter()
        .filter(|q| if item.questions.is_empty() { q.zondag == item.zondag } else { item.questions.contains(&q.nummer) })
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_questions() {
        assert_eq!(ALL.len(), 129);
        let q = for_item(&Item { questions: vec![57], ..Default::default() });
        assert_eq!(q[0].zondag, 22);
        assert!(q[0].antwoord.starts_with("Dat niet alleen mijn ziel na dit leven van stonden aan tot Christus"));
        let z = for_item(&Item { zondag: 22, ..Default::default() });
        assert_eq!(z.iter().map(|q| q.nummer).collect::<Vec<_>>(), vec![57, 58]);
    }
}
