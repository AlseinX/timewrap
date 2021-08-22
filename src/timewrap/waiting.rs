use core::cmp::Ordering;

use super::*;

pub struct Waiting<T>
where
    T: Time,
{
    pub id: usize,
    pub time: T,
}

impl<T: Time> PartialEq for Waiting<T> {
    fn eq(&self, other: &Self) -> bool {
        self.time == other.time
    }
}

impl<T: Time> Eq for Waiting<T> {}

impl<T: Time> PartialOrd for Waiting<T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.time.partial_cmp(&other.time).map(Ordering::reverse)
    }
}

impl<T: Time> Ord for Waiting<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.time.cmp(&other.time).reverse()
    }
}
