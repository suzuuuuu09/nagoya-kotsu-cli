use crate::{
    cli::Docs,
    error::Error,
    model::{Data, ResultData},
};
use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Document {
    pub name: &'static str,
    pub summary: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<&'static str>,
}

const DOCUMENTS: [Document; 5] = [
    Document {
        name: "bus",
        summary: "市バスの停留所・時刻表・接近情報",
        content: Some(include_str!("../docs/cli/bus.md")),
    },
    Document {
        name: "subway",
        summary: "地下鉄の時刻表・予定列車",
        content: Some(include_str!("../docs/cli/subway.md")),
    },
    Document {
        name: "route",
        summary: "経路検索と交通手段・時刻の指定",
        content: Some(include_str!("../docs/cli/route.md")),
    },
    Document {
        name: "output",
        summary: "JSON・raw・終了コード・キャッシュ",
        content: Some(include_str!("../docs/cli/output.md")),
    },
    Document {
        name: "troubleshooting",
        summary: "入力・通信・解析エラーへの対処",
        content: Some(include_str!("../docs/cli/troubleshooting.md")),
    },
];

pub fn run(command: &Docs) -> Result<ResultData, Error> {
    let data = match command {
        Docs::List => Data::Documents(
            DOCUMENTS
                .iter()
                .map(|d| Document {
                    content: None,
                    ..*d
                })
                .collect(),
        ),
        Docs::Show { name } => {
            Data::Document(*DOCUMENTS.iter().find(|d| d.name == name).ok_or_else(|| {
                Error::NotFound {
                    name: name.clone(),
                    candidates: DOCUMENTS
                        .iter()
                        .map(|d| d.name)
                        .collect::<Vec<_>>()
                        .join("、"),
                }
            })?)
        }
    };
    Ok(ResultData::new(data))
}
