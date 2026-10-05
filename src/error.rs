use crate::cli::PlaceType;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct PlaceCandidate {
    pub name: String,
    pub qualified_name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("「{name}」は見つかりませんでした")]
    PlaceNotFound {
        name: String,
        kind: Option<PlaceType>,
    },
    #[error("「{name}」に複数の交通施設の候補があります")]
    PlaceAmbiguous {
        name: String,
        candidates: Vec<PlaceCandidate>,
    },
    #[error("施設「{name}」は見つかりましたが、{reason}。代表座標・周辺検索には利用できません")]
    CoordinatesUnavailable {
        name: String,
        kind: PlaceType,
        reason: String,
    },
    #[error("{scope}: {source}")]
    Scoped { scope: String, source: Box<Error> },
    #[error("名古屋市交通局に接続できませんでした")]
    Network(String),
    #[error("名古屋市交通局からHTTPエラーが返されました ({status})")]
    Http { status: u16, url: String },
    #[error("名古屋市交通局から不正なレスポンスが返されました")]
    InvalidResponse(String),
    #[error("名古屋市交通局のレスポンスを解析できませんでした")]
    Parse(String),
    #[error("「{name}」は見つかりませんでした\n候補: {candidates}")]
    NotFound { name: String, candidates: String },
    #[error(
        "「{name}」に複数の候補があります\n候補: {candidates}\n候補をより具体的に指定してください"
    )]
    Ambiguous { name: String, candidates: String },
    #[error("名古屋市交通局のAPIがエラーを返しました ({0})")]
    Upstream(String),
    #[error("キャッシュを利用できませんでした。--no-cache を指定して再実行できます")]
    Cache(String),
    #[error("{0}")]
    Arguments(String),
}

impl Error {
    pub fn scoped(self, scope: impl Into<String>) -> Self {
        Self::Scoped {
            scope: scope.into(),
            source: Box::new(self),
        }
    }
    pub fn source_error(&self) -> &Self {
        match self {
            Self::Scoped { source, .. } => source.source_error(),
            _ => self,
        }
    }
    pub fn code(&self) -> &'static str {
        match self {
            Self::Scoped { source, .. } => source.code(),
            Self::Arguments(_) => "invalid_arguments",
            Self::NotFound { .. } | Self::PlaceNotFound { .. } => "not_found",
            Self::Ambiguous { .. } | Self::PlaceAmbiguous { .. } => "ambiguous",
            Self::Network(_) => "network_error",
            Self::Http { .. } => "http_error",
            Self::InvalidResponse(_) | Self::CoordinatesUnavailable { .. } => "invalid_response",
            Self::Parse(_) => "parse_error",
            Self::Upstream(_) => "upstream_error",
            Self::Cache(_) => "cache_error",
        }
    }
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Scoped { source, .. } => source.exit_code(),
            Self::Arguments(_) => 2,
            Self::NotFound { .. }
            | Self::Ambiguous { .. }
            | Self::PlaceNotFound { .. }
            | Self::PlaceAmbiguous { .. } => 3,
            Self::Network(_) | Self::Http { .. } => 4,
            Self::InvalidResponse(_) | Self::Parse(_) | Self::CoordinatesUnavailable { .. } => 5,
            Self::Upstream(_) => 6,
            Self::Cache(_) => 1,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Failure {
    pub scope: String,
    pub code: &'static str,
    pub exit_code: u8,
    pub message: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub candidates: Vec<PlaceCandidate>,
    #[serde(skip)]
    pub detail: String,
}

impl Failure {
    pub fn new(scope: impl Into<String>, error: Error) -> Self {
        let scope = match &error {
            Error::Scoped { scope, .. } => scope.clone(),
            _ => scope.into(),
        };
        let candidates = match error.source_error() {
            Error::PlaceAmbiguous { candidates, .. } => candidates.clone(),
            _ => Vec::new(),
        };
        Self {
            scope,
            candidates,
            code: error.code(),
            exit_code: error.exit_code(),
            message: error.to_string(),
            detail: format!("{error:?}"),
        }
    }
}
