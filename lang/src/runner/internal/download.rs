use std::{sync::Arc, time::Instant};

use chrono::Utc;
use indexmap::IndexMap;

use crate::{
    driver::{DownloadStatus, DriverTrait, monitoring::Call},
    runner::{
        ResultValue,
        cache::{CacheEntry, CacheKey, CacheTrait},
        dep::Dep,
        dep_list::DepList,
        error::RunnerError,
        internal::InternalCx,
        value::{RainInteger, RainRecord, RainTypeId, Value},
    },
};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn download(self) -> ResultValue {
        self.deps.push(Dep::Download);
        match &self.arg_values[..] {
            [(url_nid, url_value)] => {
                let start = Instant::now();
                let Value::String(url) = url_value else {
                    return Err(self.caller_cx.nid_err(
                        *url_nid,
                        RunnerError::ExpectedType {
                            actual: url_value.rain_type_id(),
                            expected: std::borrow::Cow::Borrowed(&[RainTypeId::String]),
                        },
                    ));
                };
                let cache_key = CacheKey::Download {
                    url: url.to_string(),
                };
                let _call = self
                    .runner
                    .driver
                    .call_guard(Call::Custom(format!("Download {url}")));
                let cache_entry = self.runner.cache.get(
                    &cache_key,
                    self.runner.driver,
                    &mut self.runner.local_file_hash_cache,
                );
                if let Some(cache_entry) = &cache_entry {
                    if let Some(expires) = cache_entry.expires {
                        if expires > Utc::now() || self.runner.offline {
                            log::debug!("Download cache hit, not expired");
                            return Ok(cache_entry.value.clone());
                        }
                    } else {
                        log::debug!("Download cache hit, no expiry");
                        return Ok(cache_entry.value.clone());
                    }
                }
                if self.runner.offline {
                    return Err(self.caller_cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift(
                            "offline mode: cannot download item is not in cache".into(),
                        ),
                    ));
                }
                log::debug!("Download cache miss");
                let etag: Option<&[u8]> = cache_entry.as_ref().and_then(|e| e.etag.as_deref());
                let DownloadStatus {
                    ok,
                    status_code,
                    file,
                    etag,
                } = self
                    .runner
                    .driver
                    .download(url, "download", etag)
                    .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
                if !ok && status_code == Some(304) {
                    // Etag matched we can use our cached value!
                    if let Some(mut cache_entry) = cache_entry {
                        log::debug!("Download cache etag hit");
                        // TODO: Maybe we shouldn't have an expiry on this?
                        cache_entry.expires = Some(Utc::now() + chrono::TimeDelta::days(30));
                        let value = cache_entry.value.clone();
                        self.runner.cache.put(cache_key, cache_entry);
                        return Ok(value);
                    }
                }
                let mut m = IndexMap::new();
                m.insert("ok".to_owned(), Value::Boolean(ok));
                m.insert(
                    "status_code".to_owned(),
                    Value::Integer(Arc::new(RainInteger(
                        status_code.unwrap_or_default().into(),
                    ))),
                );
                if let Some(file) = file {
                    m.insert("file".to_owned(), Value::GeneratedFile(Arc::new(file)));
                } else {
                    m.insert("file".to_owned(), Value::Unit);
                }
                let out = Value::Record(Arc::new(RainRecord(m)));
                self.runner.cache.put(
                    cache_key,
                    CacheEntry {
                        execution_time: start.elapsed(),
                        etag,
                        expires: Some(Utc::now() + chrono::TimeDelta::hours(1)),
                        deps: DepList::new(),
                        value: out.clone(),
                    },
                );
                Ok(out)
            }
            _ => self.incorrect_args(2..=2),
        }
    }
}
