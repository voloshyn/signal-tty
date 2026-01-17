mod stdio;

pub use stdio::StdioTransport;

use async_trait::async_trait;
use thiserror::Error;
use tokio::sync::broadcast;

#[derive(Error, Debug)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Receive failed: {0}")]
    ReceiveFailed(String),

    #[error("Connection closed")]
    ConnectionClosed,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[async_trait]
pub trait Transport: Send + Sync {
    async fn connect(&self) -> Result<(), TransportError>;
    async fn send(&self, data: &[u8]) -> Result<(), TransportError>;
    async fn receive(&self) -> Result<Vec<u8>, TransportError>;
    fn subscribe(&self) -> broadcast::Receiver<Vec<u8>>;
    fn is_connected(&self) -> bool;
    async fn disconnect(&self) -> Result<(), TransportError>;
}
