use std::{sync::Arc, time::Instant};

use chrono::Utc;
use indexmap::IndexMap;
use tracing::{debug, warn};

use crate::{
    driver::{DownloadStatus, DriverTrait, monitoring::Call},
    runner::{
        Result,
        cache::{CacheEntry, CacheKey, CacheTrait},
        dep::Dep,
        dep_list::DepList,
        error::RunnerError,
        internal::{
            InternalCx,
            macros::{expect_type, unpack_args},
        },
        value::{RainInteger, RainRecord, RainTypeId, Value},
    },
};

impl<Driver: DriverTrait, Cache: CacheTrait> InternalCx<'_, '_, '_, Driver, Cache> {
    pub fn download(self) -> Result<Value> {
        self.deps.push(Dep::Download);
        let (url_nid, url_value) = unpack_args!(self, 1);
        let start = Instant::now();
        let Value::String(url) = url_value else {
            return Err(self.caller_cx.nid_err(
                url_nid,
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
            let Some(expires) = cache_entry.expires else {
                debug!("Download cache hit, no expiry");
                return Ok(cache_entry.value.clone());
            };
            if expires > Utc::now() {
                debug!("Download cache hit, not expired");
                return Ok(cache_entry.value.clone());
            }
            if self.runner.offline {
                debug!("Download cache hit, expired but offline mode");
                return Ok(cache_entry.value.clone());
            }
            debug!("Download cache miss because expired");
        }
        if self.runner.offline {
            return Err(self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift("offline mode: cannot download item is not in cache".into()),
            ));
        }
        debug!("Download cache miss");
        let etag: Option<&[u8]> = cache_entry.as_ref().and_then(|e| e.etag.as_deref());
        let DownloadStatus {
            status_code,
            file,
            etag,
        } = self
            .runner
            .driver
            .http_download(url, etag)
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;
        if status_code == 304 {
            // Etag matched we can use our cached value!
            if let Some(cache_entry) = cache_entry {
                debug!("Download cache etag hit");
                return Ok(cache_entry.value);
            }
        }
        let mut m = IndexMap::new();
        m.insert("ok".to_owned(), Value::Boolean(status_code == 200));
        m.insert(
            "status_code".to_owned(),
            Value::Integer(Arc::new(RainInteger(status_code.into()))),
        );
        m.insert("file".to_owned(), Value::GeneratedFile(Arc::new(file)));
        if etag.is_none() {
            warn!("no etag provided for download can result in more cache misses");
        }
        let out = Value::Record(Arc::new(RainRecord(m)));
        self.runner.cache.put(
            cache_key,
            CacheEntry {
                execution_time: start.elapsed(),
                etag,
                expires: Some(Utc::now() + chrono::TimeDelta::hours(4)),
                deps: DepList::new(),
                value: out.clone(),
            },
        );
        Ok(out)
    }

    pub fn http_post(self) -> Result<Value> {
        if self.runner.offline {
            return Err(self.caller_cx.nid_err(
                self.nid,
                RunnerError::Makeshift("offline mode: cannot http post".into()),
            ));
        }
        let (url, headers, body) = unpack_args!(self, 3);
        let url = expect_type!(self, String, url);
        let url = url.to_string();
        let headers_nid = headers.0;
        let headers = expect_type!(self, List, headers);
        let mut headers_out: Vec<(String, String)> = Vec::new();
        for h in &headers.0 {
            let h = expect_type!(self, List, (headers_nid, h));
            let [name, value] = &h.0[..] else {
                return Err(self.caller_cx.nid_err(
                    headers_nid,
                    RunnerError::Makeshift("must be exactly 2 elements".into()),
                ));
            };
            let name = expect_type!(self, String, (headers_nid, name)).to_string();
            let value = expect_type!(self, String, (headers_nid, value)).to_string();
            headers_out.push((name, value));
        }
        let body = self.expect_file(body)?;
        let response = self
            .runner
            .driver
            .http_post(crate::driver::HttpPostRequest {
                url,
                headers: headers_out,
                body,
            })
            .map_err(|err| self.caller_cx.nid_err(self.nid, err))?;

        let mut m = IndexMap::new();
        m.insert("ok".to_owned(), Value::Boolean(response.status_code == 200));
        m.insert(
            "status_code".to_owned(),
            Value::Integer(Arc::new(RainInteger(response.status_code.into()))),
        );
        m.insert(
            "body".to_owned(),
            Value::GeneratedFile(Arc::new(response.body)),
        );
        Ok(Value::Record(Arc::new(RainRecord(m))))
    }
}
