//! The mashup's own per-character data: owned artifacts, hotbar, enchantments.
//! Lives in %APPDATA%/LandsBetweenDungeons/<character>.json; Elden Ring's own saves are never touched.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct Enchant {
    pub id: String,
    pub level: u8,
}

#[derive(Serialize, Deserialize, Default, Clone, Debug)]
pub struct CharacterSave {
    pub version: u32,
    pub first_kill_done: bool,
    pub owned_artifacts: Vec<String>,
    pub hotbar: [Option<String>; 3],
    /// Enchantments per equipped item, keyed by the item's inventory (gaitem) handle.
    pub enchants: BTreeMap<u32, Vec<Enchant>>,
}

pub const MAX_ENCHANTS_PER_ITEM: usize = 3;

fn path_for(character: &str) -> PathBuf {
    let safe: String = character
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' { c } else { '_' })
        .collect();
    let safe = if safe.trim().is_empty() { "unnamed".to_string() } else { safe };
    crate::log::data_dir().join(format!("{safe}.json"))
}

pub fn load(character: &str) -> CharacterSave {
    match fs::read_to_string(path_for(character)) {
        Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
            crate::log!("save: {character}: unreadable ({e}); starting fresh");
            CharacterSave { version: 1, ..Default::default() }
        }),
        Err(_) => CharacterSave { version: 1, ..Default::default() },
    }
}

pub fn store(character: &str, save: &CharacterSave) {
    let path = path_for(character);
    let tmp = path.with_extension("json.tmp");
    match serde_json::to_string_pretty(save) {
        Ok(s) => {
            if fs::write(&tmp, s).is_ok() {
                let _ = fs::rename(&tmp, &path);
            }
        }
        Err(e) => crate::log!("save: {character}: {e}"),
    }
}

impl CharacterSave {
    /// Dungeons rule: a duplicate keeps the higher level; a full item replaces its lowest enchantment.
    pub fn add_enchant(&mut self, item: u32, id: &str, level: u8) -> bool {
        let list = self.enchants.entry(item).or_default();
        if let Some(e) = list.iter_mut().find(|e| e.id == id) {
            if level > e.level {
                e.level = level;
                return true;
            }
            return false;
        }
        if list.len() >= MAX_ENCHANTS_PER_ITEM {
            if let Some((i, _)) = list.iter().enumerate().min_by_key(|(_, e)| e.level) {
                list.remove(i);
            }
        }
        list.push(Enchant { id: id.to_string(), level });
        true
    }

    pub fn level_of(&self, item: Option<u32>, id: &str) -> u8 {
        item.and_then(|h| self.enchants.get(&h))
            .and_then(|l| l.iter().find(|e| e.id == id))
            .map_or(0, |e| e.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enchant_rules() {
        let mut s = CharacterSave::default();
        assert!(s.add_enchant(7, "sharpness", 1));
        assert!(!s.add_enchant(7, "sharpness", 1));
        assert!(s.add_enchant(7, "sharpness", 3));
        assert_eq!(s.level_of(Some(7), "sharpness"), 3);
        s.add_enchant(7, "echo", 1);
        s.add_enchant(7, "committed", 2);
        s.add_enchant(7, "swirling", 2);
        let l = &s.enchants[&7];
        assert_eq!(l.len(), 3);
        assert!(l.iter().all(|e| e.id != "echo"), "lowest level is replaced");
    }

    #[test]
    fn roundtrip() {
        let mut s = CharacterSave::default();
        s.owned_artifacts.push("wind_horn".into());
        s.hotbar[1] = Some("wind_horn".into());
        s.add_enchant(42, "thorns", 2);
        let j = serde_json::to_string(&s).unwrap();
        let back: CharacterSave = serde_json::from_str(&j).unwrap();
        assert_eq!(back.hotbar[1].as_deref(), Some("wind_horn"));
        assert_eq!(back.level_of(Some(42), "thorns"), 2);
    }
}
