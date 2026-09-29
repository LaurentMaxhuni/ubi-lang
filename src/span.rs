#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Span {
    pub(crate) source_id: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
