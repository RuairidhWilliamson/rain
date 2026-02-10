/// SHA256 hash of a file's contents
#[derive(Hash, Clone, PartialEq, Eq)]
pub struct FileHash(pub [u8; 32]);

impl std::fmt::Debug for FileHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Print in hex
        f.write_str(&base16::encode_lower(&self.0))
    }
}
