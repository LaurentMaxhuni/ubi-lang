use std::collections::BTreeMap;

use crate::lexer::LexError;
use crate::span::Span;

pub(crate) const MAX_SOURCE_BYTES: usize = 1_048_576;
pub(crate) const MAX_PROJECT_BYTES: usize = 4_194_304;
pub(crate) const MAX_MODULES: usize = 64;
pub(crate) const MAX_TOKENS: usize = 500_000;
pub(crate) const MAX_NESTING: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SourceFile {
    id: String,
    bytes: Vec<u8>,
    revision: String,
}

impl SourceFile {
    pub(crate) fn new(id: &str, bytes: Vec<u8>) -> Result<Self, SourceError> {
        if !valid_source_id(id) {
            return Err(SourceError::InvalidId(id.to_owned()));
        }
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(SourceError::Diagnostic(resource_error(id)));
        }

        let revision = format!("sha256:{}", hex(&sha256(&bytes)));
        Ok(Self {
            id: id.to_owned(),
            bytes,
            revision,
        })
    }

    pub(crate) fn id(&self) -> &str {
        &self.id
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(crate) fn revision(&self) -> &str {
        &self.revision
    }

    pub(crate) fn text(&self) -> Result<&str, LexError> {
        if self.bytes.starts_with(&[0xef, 0xbb, 0xbf]) {
            return Err(LexError {
                code: "UBI0001".to_owned(),
                message: "UTF-8 byte-order marks are not permitted".to_owned(),
                primary: Span {
                    source_id: self.id.clone(),
                    start: 0,
                    end: 3,
                },
            });
        }

        match std::str::from_utf8(&self.bytes) {
            Ok(text) => Ok(text),
            Err(error) => {
                let start = error.valid_up_to();
                let end = start + error.error_len().unwrap_or(self.bytes.len() - start);
                Err(LexError {
                    code: "UBI0001".to_owned(),
                    message: "Source is not valid UTF-8".to_owned(),
                    primary: Span {
                        source_id: self.id.clone(),
                        start,
                        end,
                    },
                })
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SourceError {
    InvalidId(String),
    DuplicateId(String),
    Diagnostic(LexError),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub(crate) struct SourceSet {
    files: BTreeMap<String, SourceFile>,
}

impl SourceSet {
    pub(crate) fn insert(&mut self, file: SourceFile) -> Result<(), SourceError> {
        if self.files.contains_key(file.id()) {
            return Err(SourceError::DuplicateId(file.id));
        }
        let project_bytes: usize = self.files.values().map(|source| source.bytes.len()).sum();
        if self.files.len() >= MAX_MODULES || project_bytes + file.bytes.len() > MAX_PROJECT_BYTES {
            return Err(SourceError::Diagnostic(resource_error(file.id())));
        }
        self.files.insert(file.id.clone(), file);
        Ok(())
    }

    pub(crate) fn get(&self, id: &str) -> Option<&SourceFile> {
        self.files.get(id)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &SourceFile> {
        self.files.values()
    }
}

fn resource_error(id: &str) -> LexError {
    LexError {
        code: "UBI0090".to_owned(),
        message: "Compiler resource limit exceeded".to_owned(),
        primary: Span {
            source_id: id.to_owned(),
            start: 0,
            end: 0,
        },
    }
}

fn valid_source_id(id: &str) -> bool {
    id.ends_with(".ubi")
        && !id.starts_with('/')
        && !id.contains('\\')
        && id.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.contains(':')
        })
}

pub(crate) fn resolve_import_id(importer_id: &str, path: &str) -> Result<String, ()> {
    if !(path.starts_with("./") || path.starts_with("../"))
        || path.contains('\\')
        || !path.ends_with(".ubi")
    {
        return Err(());
    }

    let mut segments: Vec<&str> = importer_id.split('/').collect();
    segments.pop();
    for segment in path.split('/') {
        match segment {
            "" => return Err(()),
            "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(());
                }
            }
            value => segments.push(value),
        }
    }

    let id = segments.join("/");
    if valid_source_id(&id) {
        Ok(id)
    } else {
        Err(())
    }
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

fn sha256(input: &[u8]) -> [u8; 32] {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];
    let mut state: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (input.len() as u64).wrapping_mul(8);
    let mut padded = Vec::with_capacity(input.len() + 72);
    padded.extend_from_slice(input);
    padded.push(0x80);
    while padded.len() % 64 != 56 {
        padded.push(0);
    }
    padded.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in padded.chunks_exact(64) {
        let mut schedule = [0u32; 64];
        for (index, word) in chunk.chunks_exact(4).take(16).enumerate() {
            schedule[index] = u32::from_be_bytes(word.try_into().expect("four-byte word"));
        }
        for index in 16..64 {
            let x = schedule[index - 15];
            let y = schedule[index - 2];
            let sigma0 = x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3);
            let sigma1 = y.rotate_right(17) ^ y.rotate_right(19) ^ (y >> 10);
            schedule[index] = schedule[index - 16]
                .wrapping_add(sigma0)
                .wrapping_add(schedule[index - 7])
                .wrapping_add(sigma1);
        }

        let [mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h] = state;
        for index in 0..64 {
            let big1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let choose = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(big1)
                .wrapping_add(choose)
                .wrapping_add(K[index])
                .wrapping_add(schedule[index]);
            let big0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let majority = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = big0.wrapping_add(majority);
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }
        for (slot, value) in state.iter_mut().zip([a, b, c, d, e, f, g, h]) {
            *slot = (*slot).wrapping_add(value);
        }
    }

    let mut digest = [0u8; 32];
    for (chunk, word) in digest.chunks_exact_mut(4).zip(state) {
        chunk.copy_from_slice(&word.to_be_bytes());
    }
    digest
}
