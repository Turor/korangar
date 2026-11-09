//! Local clan info and online counts.

use korangar_interface::element::StateElement;
use rust_state::RustState;

/// Fields from `ZC_CLANINFO` (`0x098A`).
pub struct ClanInfoUpdate {
    pub clan_id: u32,
    pub name: String,
    pub master_name: String,
    pub map_name: String,
    pub alliances: Vec<String>,
    pub antagonists: Vec<String>,
}

/// Clan the local character belongs to. Empty when `joined` is false.
#[derive(Clone, Debug, Default, RustState, StateElement)]
pub struct ClanState {
    pub joined: bool,
    pub clan_id: u32,
    pub name: String,
    pub master_name: String,
    pub map_name: String,
    pub alliances: Vec<String>,
    pub antagonists: Vec<String>,
    pub online_members: u16,
    pub maximum_members: u16,
    /// Preformatted rows so the window can borrow them for the frame.
    pub rows: Vec<String>,
}

impl ClanState {
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn refresh_rows(&mut self) {
        let mut rows = Vec::new();
        rows.push(format!("Name: {}", self.name));
        rows.push(format!("Master: {}", self.master_name));
        rows.push(format!("Map: {}", self.map_name));
        rows.push(format!("Online: {} / {}", self.online_members, self.maximum_members));
        rows.push("Alliances".to_owned());
        if self.alliances.is_empty() {
            rows.push("None".to_owned());
        } else {
            for name in &self.alliances {
                rows.push(name.clone());
            }
        }
        rows.push("Antagonists".to_owned());
        if self.antagonists.is_empty() {
            rows.push("None".to_owned());
        } else {
            for name in &self.antagonists {
                rows.push(name.clone());
            }
        }
        self.rows = rows;
    }

    pub fn apply_info(&mut self, info: ClanInfoUpdate) {
        if info.clan_id == 0 {
            self.clear();
            return;
        }
        if self.clan_id != 0 && self.clan_id != info.clan_id {
            self.online_members = 0;
            self.maximum_members = 0;
        }
        self.joined = true;
        self.clan_id = info.clan_id;
        self.name = info.name;
        self.master_name = info.master_name;
        self.map_name = info.map_name;
        self.alliances = info.alliances;
        self.antagonists = info.antagonists;
        self.refresh_rows();
    }

    pub fn set_online_count(&mut self, online_members: u16, maximum_members: u16) {
        self.online_members = online_members;
        self.maximum_members = maximum_members;
        if self.joined {
            self.refresh_rows();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swords() -> ClanInfoUpdate {
        ClanInfoUpdate {
            clan_id: 3,
            name: "Swords".to_owned(),
            master_name: "Lead".to_owned(),
            map_name: "prontera".to_owned(),
            alliances: vec!["Archers".to_owned()],
            antagonists: vec!["Axes".to_owned()],
        }
    }

    #[test]
    fn zero_clan_id_clears() {
        let mut clan = ClanState::default();
        clan.apply_info(swords());
        clan.set_online_count(4, 20);
        clan.apply_info(ClanInfoUpdate {
            clan_id: 0,
            name: String::new(),
            master_name: String::new(),
            map_name: String::new(),
            alliances: Vec::new(),
            antagonists: Vec::new(),
        });
        assert!(!clan.joined);
        assert!(clan.name.is_empty());
        assert!(clan.alliances.is_empty());
        assert_eq!(clan.online_members, 0);
    }

    #[test]
    fn online_count_updates_rows() {
        let mut clan = ClanState::default();
        clan.apply_info(swords());
        clan.set_online_count(4, 20);
        assert_eq!(clan.online_members, 4);
        assert_eq!(clan.maximum_members, 20);
        assert!(clan.rows.iter().any(|row| row == "Online: 4 / 20"));
        assert!(clan.rows.iter().any(|row| row == "Archers"));
    }

    #[test]
    fn switching_clan_drops_lists_and_counts() {
        let mut clan = ClanState::default();
        clan.apply_info(swords());
        clan.set_online_count(4, 20);
        clan.apply_info(ClanInfoUpdate {
            clan_id: 8,
            name: "Axes".to_owned(),
            master_name: "Other".to_owned(),
            map_name: "geffen".to_owned(),
            alliances: Vec::new(),
            antagonists: vec!["Swords".to_owned()],
        });
        assert_eq!(clan.name, "Axes");
        assert!(clan.alliances.is_empty());
        assert_eq!(clan.antagonists, vec!["Swords".to_owned()]);
        assert_eq!(clan.online_members, 0);
        assert_eq!(clan.maximum_members, 0);
    }
}
