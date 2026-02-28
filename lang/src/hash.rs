use serde::{Deserialize, Serialize};

/// SHA256 hash of a file's contents
#[derive(Hash, Clone, PartialOrd, Ord, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash(pub [u8; 32]);

impl std::fmt::Debug for FileHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print in hex
        f.write_fmt(format_args!("FileHash({})", base16::encode_lower(&self.0)))
    }
}

impl std::fmt::Display for FileHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print in hex
        f.write_str(&base16::encode_lower(&self.0))
    }
}
