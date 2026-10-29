#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Span {
    pub source_id: String,
    pub start: usize,
    pub end: usize,
}
