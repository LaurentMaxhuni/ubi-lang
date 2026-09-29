#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Span {
    pub source_id: String,
    pub start: usize,
    pub end: usize,
}
