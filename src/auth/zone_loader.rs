use crate::auth::memory_zone::MemoryZone;
use crate::auth::zone::ZoneUpdater;
use anyhow::Result;
use r53::{Name, RRset};
use std::str::FromStr;

pub fn load_zone(name: Name, content: &str) -> Result<MemoryZone> {
    let mut zone = MemoryZone::new(name);
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let rrset = RRset::from_str(line)?;
        zone.add_rrset(rrset)?;
    }
    Ok(zone)
}
