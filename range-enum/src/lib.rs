#![no_std]

use core::ops::RangeBounds;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnyRange<T> {
    Range(core::ops::Range<T>),
    RangeFrom(core::ops::RangeFrom<T>),
    RangeFull(core::ops::RangeFull),
    RangeInclusive(core::ops::RangeInclusive<T>),
    RangeTo(core::ops::RangeTo<T>),
    RangeToInclusive(core::ops::RangeToInclusive<T>),
}

impl<T> From<core::ops::Range<T>> for AnyRange<T> {
    fn from(value: core::ops::Range<T>) -> Self {
        Self::Range(value)
    }
}

impl<T> From<core::ops::RangeFrom<T>> for AnyRange<T> {
    fn from(value: core::ops::RangeFrom<T>) -> Self {
        Self::RangeFrom(value)
    }
}

impl<T> From<core::ops::RangeFull> for AnyRange<T> {
    fn from(value: core::ops::RangeFull) -> Self {
        Self::RangeFull(value)
    }
}

impl<T> From<core::ops::RangeInclusive<T>> for AnyRange<T> {
    fn from(value: core::ops::RangeInclusive<T>) -> Self {
        Self::RangeInclusive(value)
    }
}

impl<T> From<core::ops::RangeTo<T>> for AnyRange<T> {
    fn from(value: core::ops::RangeTo<T>) -> Self {
        Self::RangeTo(value)
    }
}

impl<T> From<core::ops::RangeToInclusive<T>> for AnyRange<T> {
    fn from(value: core::ops::RangeToInclusive<T>) -> Self {
        Self::RangeToInclusive(value)
    }
}

impl<T> RangeBounds<T> for AnyRange<T> {
    fn start_bound(&self) -> core::ops::Bound<&T> {
        match self {
            AnyRange::Range(value) => value.start_bound(),
            AnyRange::RangeFrom(value) => value.start_bound(),
            AnyRange::RangeFull(value) => value.start_bound(),
            AnyRange::RangeInclusive(value) => value.start_bound(),
            AnyRange::RangeTo(value) => value.start_bound(),
            AnyRange::RangeToInclusive(value) => value.start_bound(),
        }
    }

    fn end_bound(&self) -> core::ops::Bound<&T> {
        match self {
            AnyRange::Range(value) => value.end_bound(),
            AnyRange::RangeFrom(value) => value.end_bound(),
            AnyRange::RangeFull(value) => value.end_bound(),
            AnyRange::RangeInclusive(value) => value.end_bound(),
            AnyRange::RangeTo(value) => value.end_bound(),
            AnyRange::RangeToInclusive(value) => value.end_bound(),
        }
    }
}
