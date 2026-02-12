use thiserror::Error;

//create types errors for easy testability

#[derive(Error, Debug)]
pub enum CollectorError {
    #[error("failed to read {path}: {source}")]
    ProcReadError {
        path: String,
        source: std::io::Error,
    },

    #[error("failed to parse {field} from {path}: {raw}")]
    ParseError {
        path: String,
        field: String,
        raw: String,
    },

    #[error("process {pid} disappeared during collection")]
    ProcessVanished { pid: u32 },

    #[error("collection timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
}
