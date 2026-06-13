use std::collections::HashMap;

/// An entry in the OSPF Link State Database.
#[derive(Clone, Debug)]
pub struct LsaEntry {
    pub link_state_id: String,
    pub advertising_router: String,
    pub sequence_number: u32,
    pub age_secs: u16,
    pub lsa_type: LsaType,
    pub metric: Option<u32>,
    pub area: Option<String>,
}

/// OSPF LSA types.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LsaType {
    Router,
    Network,
    Summary,
    AsExternal,
}

/// The OSPF Link State Database.
#[derive(Clone, Debug, Default)]
pub struct Lsdb {
    lsas: HashMap<String, LsaEntry>,
}

impl Lsdb {
    pub fn insert(&mut self, lsa: LsaEntry) {
        self.lsas.insert(lsa.link_state_id.clone(), lsa);
    }

    pub fn get(&self, id: &str) -> Option<&LsaEntry> {
        self.lsas.get(id)
    }

    pub fn len(&self) -> usize {
        self.lsas.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &LsaEntry)> {
        self.lsas.iter()
    }
}
