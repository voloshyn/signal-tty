use super::{Transport, TransportError};
use async_trait::async_trait;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, Mutex};
use tracing::{debug, error, info};

pub struct StdioTransport {
    account: Option<String>,
    child: Arc<Mutex<Option<Child>>>,
    stdin: Arc<Mutex<Option<tokio::process::ChildStdin>>>,
    sender: broadcast::Sender<Vec<u8>>,
    connected: AtomicBool,
}

impl StdioTransport {
    pub fn new(account: Option<String>) -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            account,
            child: Arc::new(Mutex::new(None)),
            stdin: Arc::new(Mutex::new(None)),
            sender,
            connected: AtomicBool::new(false),
        }
    }

    fn spawn_reader(
        stdout: tokio::process::ChildStdout,
        sender: broadcast::Sender<Vec<u8>>,
        connected: Arc<AtomicBool>,
    ) {
        tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();

            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        debug!("Received: {}", line);
                        if sender.send(line.into_bytes()).is_err() {}
                    }
                    Ok(None) => {
                        info!("signal-cli process ended");
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                    Err(e) => {
                        error!("Read error: {}", e);
                        connected.store(false, Ordering::SeqCst);
                        break;
                    }
                }
            }
        });
    }
}

#[async_trait]
impl Transport for StdioTransport {
    async fn connect(&self) -> Result<(), TransportError> {
        let mut cmd = Command::new("signal-cli");
        
        if let Some(ref account) = self.account {
            info!("Starting signal-cli jsonRpc for account: {}", account);
            cmd.arg("-a").arg(account);
        } else {
            info!("Starting signal-cli jsonRpc");
        }
        
        let mut child = cmd
            .arg("jsonRpc")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| TransportError::ConnectionFailed(format!("Failed to spawn signal-cli: {}", e)))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| TransportError::ConnectionFailed("Failed to get stdin".into()))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| TransportError::ConnectionFailed("Failed to get stdout".into()))?;

        {
            let mut stdin_guard = self.stdin.lock().await;
            *stdin_guard = Some(stdin);
        }

        {
            let mut child_guard = self.child.lock().await;
            *child_guard = Some(child);
        }

        let connected = Arc::new(AtomicBool::new(true));
        Self::spawn_reader(stdout, self.sender.clone(), connected.clone());

        self.connected.store(true, Ordering::SeqCst);
        info!("signal-cli jsonRpc started successfully");
        Ok(())
    }

    async fn send(&self, data: &[u8]) -> Result<(), TransportError> {
        let mut guard = self.stdin.lock().await;
        let stdin = guard
            .as_mut()
            .ok_or(TransportError::ConnectionClosed)?;

        stdin.write_all(data).await?;
        stdin.write_all(b"\n").await?;
        stdin.flush().await?;

        debug!("Sent: {}", String::from_utf8_lossy(data));
        Ok(())
    }

    async fn receive(&self) -> Result<Vec<u8>, TransportError> {
        let mut rx = self.sender.subscribe();
        rx.recv()
            .await
            .map_err(|_| TransportError::ConnectionClosed)
    }

    fn subscribe(&self) -> broadcast::Receiver<Vec<u8>> {
        self.sender.subscribe()
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn disconnect(&self) -> Result<(), TransportError> {
        {
            let mut stdin_guard = self.stdin.lock().await;
            *stdin_guard = None;
        }

        {
            let mut child_guard = self.child.lock().await;
            if let Some(mut child) = child_guard.take() {
                tokio::select! {
                    _ = child.wait() => {
                        info!("signal-cli exited gracefully");
                    }
                    _ = tokio::time::sleep(std::time::Duration::from_secs(2)) => {
                        info!("Killing signal-cli");
                        let _ = child.kill().await;
                    }
                }
            }
        }

        self.connected.store(false, Ordering::SeqCst);
        info!("Disconnected from signal-cli");
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {}
}
