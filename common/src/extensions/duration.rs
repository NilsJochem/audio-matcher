use std::time::Duration;

/// extention function for [Duration]
pub trait Ext {
    /// builds a [Duration] from the given data
    fn from_h_m_s_m(hours: u64, minutes: u64, seconds: u64, millis: u32) -> Duration;
    /// returns the hours represented by this `self`
    fn hours(&self) -> u64;
    /// returns the minuets represented by this `self`
    fn minutes(&self) -> u64;
    /// returns the seconds represented by this `self`
    fn seconds(&self) -> u64;
}

impl Ext for Duration {
    fn from_h_m_s_m(hours: u64, minutes: u64, seconds: u64, millis: u32) -> Duration {
        Self::new(hours * 3600 + minutes * 60 + seconds, millis * 1_000_000)
    }
    #[inline]
    fn hours(&self) -> u64 {
        self.as_secs() / 3600
    }
    #[inline]
    fn minutes(&self) -> u64 {
        (self.as_secs() / 60) % 60
    }
    #[inline]
    fn seconds(&self) -> u64 {
        self.as_secs() % 60
    }
}
