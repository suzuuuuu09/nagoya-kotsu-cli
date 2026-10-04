use serde::Serialize;

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
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
    pub fn code(&self) -> &'static str {
        match self {
            Self::Arguments(_) => "invalid_arguments",
            Self::NotFound { .. } => "not_found",
            Self::Ambiguous { .. } => "ambiguous",
            Self::Network(_) => "network_error",
            Self::Http { .. } => "http_error",
            Self::InvalidResponse(_) => "invalid_response",
            Self::Parse(_) => "parse_error",
            Self::Upstream(_) => "upstream_error",
            Self::Cache(_) => "cache_error",
        }
    }
    pub fn exit_code(&self) -> u8 {
        match self {
            Self::Arguments(_) => 2,
            Self::NotFound { .. } | Self::Ambiguous { .. } => 3,
            Self::Network(_) | Self::Http { .. } => 4,
            Self::InvalidResponse(_) | Self::Parse(_) => 5,
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
    #[serde(skip)]
    pub detail: String,
}

impl Failure {
    pub fn new(scope: impl Into<String>, error: Error) -> Self {
        Self {
            scope: scope.into(),
            code: error.code(),
            exit_code: error.exit_code(),
            message: error.to_string(),
            detail: format!("{error:?}"),
        }
    }
}
