pub mod cache;
use crate::{cli::Cli, error::Error};
use cache::{Cache, Record};
use reqwest::header::{
    CACHE_CONTROL, CONTENT_TYPE, ETAG, IF_MODIFIED_SINCE, IF_NONE_MATCH, LAST_MODIFIED,
};
use serde::de::DeserializeOwned;
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::{Mutex, OnceCell, Semaphore};

#[derive(Debug, Clone, Copy)]
pub enum Policy {
    Live,
    Master,
    Timetable,
}
impl Policy {
    fn ttl(self) -> i64 {
        match self {
            Self::Live => 0,
            Self::Master => 86400,
            Self::Timetable => 21600,
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub enum Format {
    Json,
    Xml,
    Script,
}
type SharedFetch = Arc<OnceCell<Result<Arc<Record>, Error>>>;

#[derive(Clone)]
pub struct ApiClient {
    inner: Arc<Inner>,
}
struct Inner {
    client: reqwest::Client,
    cache: Cache,
    refresh: bool,
    verbose: bool,
    requests: Semaphore,
    memory: Mutex<BTreeMap<String, SharedFetch>>,
    raw: Mutex<BTreeMap<String, String>>,
    base: Option<String>,
}

impl ApiClient {
    pub fn new(cli: &Cli) -> Result<Self, Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(cli.timeout))
            .user_agent(concat!("nkotsu/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|e| Error::Network(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                client,
                cache: Cache::new(cli.no_cache),
                refresh: cli.refresh,
                verbose: cli.verbose,
                requests: Semaphore::new(4),
                memory: Mutex::new(BTreeMap::new()),
                raw: Mutex::new(BTreeMap::new()),
                base: std::env::var("NKOTSU_BASE_URL").ok(),
            }),
        })
    }
    pub fn site(&self, path: &str) -> String {
        format!(
            "{}{}",
            self.inner
                .base
                .as_deref()
                .unwrap_or("https://www.kotsu.city.nagoya.jp")
                .trim_end_matches('/'),
            path
        )
    }
    pub fn map(&self, path: &str) -> String {
        format!(
            "{}{}",
            self.inner
                .base
                .as_deref()
                .unwrap_or("https://map.kotsu.city.nagoya.jp")
                .trim_end_matches('/'),
            path
        )
    }
    pub async fn raw(&self) -> BTreeMap<String, String> {
        self.inner.raw.lock().await.clone()
    }
    pub async fn json<T: DeserializeOwned + Default>(
        &self,
        url: String,
        policy: Policy,
    ) -> Result<T, Error> {
        let record = self.get(url, policy, Format::Json).await?;
        serde_json::from_str::<Option<T>>(record.body.trim_start_matches('\u{feff}'))
            .map(|v| v.unwrap_or_default())
            .map_err(|e| Error::Parse(e.to_string()))
    }
    pub async fn get(
        &self,
        url: String,
        policy: Policy,
        format: Format,
    ) -> Result<Arc<Record>, Error> {
        let cell = self
            .inner
            .memory
            .lock()
            .await
            .entry(url.clone())
            .or_default()
            .clone();
        cell.get_or_init(|| self.fetch(&url, policy, format))
            .await
            .clone()
    }
    async fn fetch(&self, url: &str, policy: Policy, format: Format) -> Result<Arc<Record>, Error> {
        let cached = if matches!(policy, Policy::Live) {
            None
        } else {
            self.inner.cache.read(url)?
        };
        let now = chrono::Utc::now().timestamp();
        if let Some(record) = &cached
            && !self.inner.refresh
            && record.expires_at > now
        {
            validate(record, format)?;
            self.inner
                .raw
                .lock()
                .await
                .insert(url.to_owned(), record.body.clone());
            return Ok(Arc::new(record.clone()));
        }
        let _permit = self
            .inner
            .requests
            .acquire()
            .await
            .map_err(|e| Error::Network(e.to_string()))?;
        for attempt in 0..3 {
            if self.inner.verbose {
                eprintln!("GET {url} (試行 {})", attempt + 1);
            }
            let mut request = self.inner.client.get(url);
            if let Some(record) = &cached {
                if let Some(etag) = &record.etag {
                    request = request.header(IF_NONE_MATCH, etag);
                }
                if let Some(date) = &record.last_modified {
                    request = request.header(IF_MODIFIED_SINCE, date);
                }
            }
            let response = match request.send().await {
                Ok(r) => r,
                Err(e) if attempt < 2 && (e.is_connect() || e.is_timeout()) => {
                    tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
                    continue;
                }
                Err(e) => return Err(Error::Network(e.to_string())),
            };
            let status = response.status();
            if matches!(status.as_u16(), 502..=504) && attempt < 2 {
                tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
                continue;
            }
            let headers = response.headers().clone();
            let header = |key| {
                headers
                    .get(key)
                    .and_then(|v| v.to_str().ok())
                    .map(str::to_owned)
            };
            let mut record = if status.as_u16() == 304 {
                cached
                    .clone()
                    .ok_or_else(|| Error::InvalidResponse("キャッシュなしの304".into()))?
            } else {
                if !status.is_success() {
                    if let Ok(bytes) = response.bytes().await
                        && let Ok(body) = String::from_utf8(bytes.to_vec())
                    {
                        self.inner.raw.lock().await.insert(url.into(), body);
                    }
                    return Err(Error::Http {
                        status: status.as_u16(),
                        url: url.into(),
                    });
                }
                let bytes = match response.bytes().await {
                    Ok(b) => b,
                    Err(e) if attempt < 2 && (e.is_connect() || e.is_timeout()) => {
                        tokio::time::sleep(Duration::from_millis(100 * (attempt + 1))).await;
                        continue;
                    }
                    Err(e) => return Err(Error::Network(e.to_string())),
                };
                let body =
                    String::from_utf8(bytes.to_vec()).map_err(|e| Error::Parse(e.to_string()))?;
                self.inner.raw.lock().await.insert(url.into(), body.clone());
                Record {
                    url: url.into(),
                    body,
                    content_type: header(CONTENT_TYPE).unwrap_or_default(),
                    etag: None,
                    last_modified: None,
                    stored_at: now,
                    expires_at: now,
                    cache_control: String::new(),
                }
            };
            if let Some(v) = header(ETAG) {
                record.etag = Some(v);
            }
            if let Some(v) = header(LAST_MODIFIED) {
                record.last_modified = Some(v);
            }
            if let Some(v) = header(CACHE_CONTROL) {
                record.cache_control = v;
            }
            let directives: Vec<_> = record.cache_control.split(',').map(str::trim).collect();
            let ttl = if directives
                .iter()
                .any(|s| s.eq_ignore_ascii_case("no-cache"))
            {
                0
            } else {
                directives
                    .iter()
                    .find_map(|s| {
                        s.to_ascii_lowercase()
                            .strip_prefix("max-age=")
                            .and_then(|n| n.trim_matches('"').parse::<i64>().ok())
                    })
                    .unwrap_or(policy.ttl())
                    .max(0)
            };
            let age = header(reqwest::header::AGE)
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0)
                .max(0);
            record.stored_at = now;
            record.expires_at = now.saturating_add(ttl.saturating_sub(age).max(0));
            validate(&record, format)?;
            if !matches!(policy, Policy::Live) {
                self.inner.cache.write(&record)?;
            }
            self.inner
                .raw
                .lock()
                .await
                .insert(url.into(), record.body.clone());
            return Ok(Arc::new(record));
        }
        Err(Error::Network("再試行回数超過".into()))
    }
}

fn validate(record: &Record, format: Format) -> Result<(), Error> {
    let mime = record
        .content_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let body = record.body.trim_start_matches('\u{feff}').trim_start();
    let accepted = match format {
        Format::Json => mime == "application/json" || mime.ends_with("+json"),
        Format::Xml => mime == "text/xml" || mime == "application/xml" || mime.ends_with("+xml"),
        Format::Script => matches!(
            mime.as_str(),
            "text/javascript"
                | "application/javascript"
                | "application/x-javascript"
                | "text/plain"
        ),
    };
    if !accepted
        || body.is_empty()
        || body.to_ascii_lowercase().starts_with("<!doctype html")
        || body.to_ascii_lowercase().starts_with("<html")
    {
        return Err(Error::InvalidResponse(format!(
            "{}: Content-Type {}",
            record.url, record.content_type
        )));
    }
    if matches!(format, Format::Json) {
        serde_json::from_str::<Box<serde_json::value::RawValue>>(body)
            .map_err(|e| Error::Parse(e.to_string()))?;
    }
    Ok(())
}
