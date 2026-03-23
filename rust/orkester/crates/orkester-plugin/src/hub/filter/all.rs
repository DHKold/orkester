use super::Filter;
use crate::hub::envelope::Envelope;

/// Matches every envelope unconditionally.
pub struct AllFilter;

impl Filter for AllFilter {
    #[inline]
    fn matches(&self, _: &Envelope) -> bool { true }
}
