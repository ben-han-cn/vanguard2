use r53::Name;
use std::{
    cmp::{Eq, PartialEq},
    fmt::{self, Debug},
    hash::{Hash, Hasher},
};

pub struct EntryKey(pub *const Name);

unsafe impl Send for EntryKey {}

impl EntryKey {
    pub fn from_name(n: &Name) -> Self {
        EntryKey(n as *const Name)
    }
}

impl Clone for EntryKey {
    fn clone(&self) -> Self {
        EntryKey(self.0)
    }
}

impl Copy for EntryKey {}

impl Debug for EntryKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe { write!(f, "{:?}, {}", self.0, *self.0) }
    }
}

impl Hash for EntryKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        unsafe {
            (*self.0).hash(state);
        }
    }
}

impl PartialEq for EntryKey {
    fn eq(&self, other: &EntryKey) -> bool {
        unsafe { (*self.0).eq(&(*other.0)) }
    }
}

impl Eq for EntryKey {}
