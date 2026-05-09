#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    #[error("invalid magic number: {0:#010x}")]
    InvalidMagic(u32),
    #[error("unsupported file version: legacy={0}, ue5={1}")]
    UnsupportedVersion(i32, u32),
    #[error("unexpected end of file")]
    UnexpectedEof,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}
