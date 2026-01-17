use crate::infrastructure::jsonrpc::RpcError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("RPC error: {0}")]
    Rpc(#[from] RpcError),

    #[error("Not connected to signal-cli daemon")]
    NotConnected,

    #[error("Account not registered")]
    NotRegistered,

    #[error("Invalid recipient: {0}")]
    InvalidRecipient(String),

    #[error("Group not found: {0}")]
    GroupNotFound(String),

    #[error("Contact not found: {0}")]
    ContactNotFound(String),

    #[error("Message send failed: {0}")]
    SendFailed(String),

    #[error("Rate limited, retry after {retry_after} seconds")]
    RateLimited { retry_after: u64 },

    #[error("Untrusted identity for {address}")]
    UntrustedIdentity { address: String },

    #[error("Proof of captcha required")]
    CaptchaRequired,

    #[error("Unknown error: {0}")]
    Unknown(String),
}
