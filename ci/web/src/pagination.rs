use std::num::NonZero;

#[derive(serde::Deserialize)]
pub struct Pagination {
    /// The page number starting at 1
    pub page: Option<NonZero<u64>>,
}

impl Pagination {
    /// Get the page number zero indexed
    pub fn page_numberz(&self) -> anyhow::Result<i64> {
        match self.page {
            Some(x) => Ok(i64::try_from(x.get())? - 1),
            None => Ok(0),
        }
    }
}

pub struct Paginated<T> {
    pub elements: Vec<T>,
    pub full_count: u64,
    // Page number starting at 1
    pub page: NonZero<u64>,
    pub page_count: u64,
}

impl<T> Paginated<T> {
    pub fn new(elements: Vec<T>, full_count: u64, per_page: u64, page: &Pagination) -> Self {
        Paginated {
            elements,
            full_count,
            page: page.page.unwrap_or(NonZero::<u64>::MIN),
            page_count: full_count.div_ceil(per_page),
        }
    }

    pub fn showing(&self) -> u64 {
        self.elements.len() as u64
    }
}
