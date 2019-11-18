use r53::{Name, RRType};
use std::{
    cmp::{Eq, PartialEq},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
};

//used as key for message and rrset cache search
pub struct EntryKey(pub *const Name, pub RRType);

unsafe impl Send for EntryKey {}

impl Clone for EntryKey {
    fn clone(&self) -> Self {
        EntryKey(self.0, self.1)
    }
}

impl Copy for EntryKey {}

impl Debug for EntryKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { write!(f, "{}:{}", (*self.0), self.1) }
    }
}

impl Hash for EntryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe {
            (*self.0).hash(state);
        }
        state.write_u16(self.1.to_u16());
    }
}

impl PartialEq for EntryKey {
    fn eq(&self, other: &EntryKey) -> bool {
        unsafe { self.1 == other.1 && (*self.0).eq(&(*other.0)) }
    }
}

impl Eq for EntryKey {}
