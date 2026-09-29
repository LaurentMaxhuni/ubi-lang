#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Span {
    pub(crate) source_id: String,
    pub(crate) start: usize,
    pub(crate) end: usize,
}
