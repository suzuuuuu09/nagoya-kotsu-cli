use crate::error::Error;
use serde::{Deserialize, Serialize};
use std::{
    collections::hash_map::DefaultHasher,
    hash::{Hash, Hasher},
    path::PathBuf,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    pub url: String,
    pub body: String,
    pub content_type: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub stored_at: i64,
    pub expires_at: i64,
    pub cache_control: String,
}

pub struct Cache {
    root: Option<PathBuf>,
}
impl Cache {
    pub fn new(disabled: bool) -> Self {
        let root = if disabled {
            None
        } else {
            std::env::var_os("NKOTSU_CACHE_DIR")
                .map(PathBuf::from)
                .or_else(|| {
                    directories::ProjectDirs::from("jp", "nagoya-kotsu", "nkotsu")
                        .map(|d| d.cache_dir().to_owned())
                })
        };
        Self { root }
    }
    fn path(&self, url: &str) -> Option<PathBuf> {
        let mut hash = DefaultHasher::new();
        url.hash(&mut hash);
        self.root
            .as_ref()
            .map(|r| r.join(format!("{:016x}.json", hash.finish())))
    }
    pub fn read(&self, url: &str) -> Result<Option<Record>, Error> {
        let Some(path) = self.path(url) else {
            return Ok(None);
        };
        let bytes = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(Error::Cache(e.to_string())),
        };
        let record: Record =
            serde_json::from_slice(&bytes).map_err(|e| Error::Cache(e.to_string()))?;
        Ok((record.url == url).then_some(record))
    }
    pub fn write(&self, record: &Record) -> Result<(), Error> {
        let Some(path) = self.path(&record.url) else {
            return Ok(());
        };
        if record
            .cache_control
            .split(',')
            .any(|d| d.trim().eq_ignore_ascii_case("no-store"))
        {
            match std::fs::remove_file(path) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => return Err(Error::Cache(e.to_string())),
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| Error::Cache(e.to_string()))?;
        }
        let temporary = path.with_extension(format!("{}.tmp", std::process::id()));
        let bytes = serde_json::to_vec(record).map_err(|e| Error::Cache(e.to_string()))?;
        std::fs::write(&temporary, bytes)
            .and_then(|_| std::fs::rename(&temporary, &path))
            .map_err(|e| Error::Cache(e.to_string()))
    }
}
