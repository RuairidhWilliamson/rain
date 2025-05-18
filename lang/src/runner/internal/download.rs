use std::{sync::Arc, time::Instant};

use chrono::Utc;
use indexmap::IndexMap;

use crate::driver::{DownloadStatus, DriverTrait};
use crate::runner::{
    ResultValue,
    cache::{CacheEntry, CacheKey},
    error::RunnerError,
    value::{RainInteger, RainRecord, RainTypeId, Value},
};

use super::{InternalCx, enter_call};

impl<D: DriverTrait> InternalCx<'_, '_, '_, '_, '_, D> {
    pub fn download(self) -> ResultValue {
        match &self.arg_values[..] {
            [(url_nid, url_value), (name_nid, name_value)] => {
                let start = Instant::now();
                let Value::String(url) = url_value else {
                    return Err(self.cx.nid_err(
                        *url_nid,
                        RunnerError::ExpectedType {
                            actual: url_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let Value::String(name) = name_value else {
                    return Err(self.cx.nid_err(
                        *name_nid,
                        RunnerError::ExpectedType {
                            actual: name_value.rain_type_id(),
                            expected: &[RainTypeId::String],
                        },
                    ));
                };
                let cache_key = CacheKey::Download {
                    url: url.to_string(),
                };
                let call_description = format!("Download {url}");
                let _call = enter_call(self.runner.driver, call_description);
                let cache_entry = self.runner.cache.get(&cache_key);
                if let Some(cache_entry) = &cache_entry {
                    if let Some(expires) = cache_entry.expires {
                        if expires > Utc::now() || self.runner.offline {
                            log::debug!("Download cache hit");
                            return Ok(cache_entry.value.clone());
                        }
                    } else {
                        log::debug!("Download cache hit");
                        return Ok(cache_entry.value.clone());
                    }
                }
                if self.runner.offline {
                    return Err(self.cx.nid_err(
                        self.nid,
                        RunnerError::Makeshift(
                            "offline mode: cannot download item is not in cache".into(),
                        ),
                    ));
                }
                let etag: Option<&str> = cache_entry.as_ref().and_then(|e| e.etag.as_deref());
                let DownloadStatus {
                    ok,
                    status_code,
                    file,
                    etag,
                } = self
                    .runner
                    .driver
                    .download(url, name, etag)
                    .map_err(|err| self.cx.nid_err(self.nid, err))?;
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
                    m.insert("file".to_owned(), Value::File(Arc::new(file)));
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
                        deps: Vec::new(),
                        value: out.clone(),
                    },
                );
                Ok(out)
            }
            _ => self.incorrect_args(2..=2),
        }
    }
}
