use super::types::{JsonRpcMessage, JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, RpcError};
use crate::infrastructure::transport::Transport;
use async_trait::async_trait;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, oneshot, Mutex};
use tracing::{debug, error, warn};
use uuid::Uuid;

#[async_trait]
pub trait RpcClient: Send + Sync {
    async fn call<P, R>(&self, method: &str, params: P) -> Result<R, RpcError>
    where
        P: Serialize + Send,
        R: DeserializeOwned;

    fn notifications(&self) -> broadcast::Receiver<JsonRpcNotification>;
}

pub struct JsonRpcClient<T: Transport> {
    transport: Arc<T>,
    pending_requests: Arc<Mutex<HashMap<String, oneshot::Sender<JsonRpcResponse>>>>,
    notification_sender: broadcast::Sender<JsonRpcNotification>,
    timeout: Duration,
}

impl<T: Transport + 'static> JsonRpcClient<T> {
    pub fn new(transport: T) -> Self {
        Self::with_timeout(transport, Duration::from_secs(30))
    }

    pub fn with_timeout(transport: T, timeout: Duration) -> Self {
        let (notification_sender, _) = broadcast::channel(256);
        Self {
            transport: Arc::new(transport),
            pending_requests: Arc::new(Mutex::new(HashMap::new())),
            notification_sender,
            timeout,
        }
    }

    pub async fn connect(&self) -> Result<(), RpcError> {
        self.transport.connect().await?;
        self.spawn_receiver();
        Ok(())
    }

    fn spawn_receiver(&self) {
        let transport = self.transport.clone();
        let pending = self.pending_requests.clone();
        let notif_sender = self.notification_sender.clone();

        tokio::spawn(async move {
            loop {
                match transport.receive().await {
                    Ok(data) => {
                        let data_str = match String::from_utf8(data) {
                            Ok(s) => s,
                            Err(e) => {
                                error!("Invalid UTF-8 in response: {}", e);
                                continue;
                            }
                        };

                        match serde_json::from_str::<JsonRpcMessage>(&data_str) {
                            Ok(JsonRpcMessage::Response(response)) => {
                                let id = response.id.clone();
                                let mut pending_guard = pending.lock().await;
                                if let Some(sender) = pending_guard.remove(&id) {
                                    if sender.send(response).is_err() {
                                        warn!("Response receiver dropped for id: {}", id);
                                    }
                                } else {
                                    warn!("No pending request for id: {}", id);
                                }
                            }
                            Ok(JsonRpcMessage::Notification(notification)) => {
                                debug!("Received notification: {}", notification.method);
                                if notif_sender.send(notification).is_err() {}
                            }
                            Err(e) => {
                                error!("Failed to parse JSON-RPC message: {} - data: {}", e, data_str);
                            }
                        }
                    }
                    Err(e) => {
                        error!("Transport receive error: {}", e);
                        break;
                    }
                }
            }
        });
    }

    fn generate_id() -> String {
        Uuid::new_v4().to_string()
    }
}

#[async_trait]
impl<T: Transport + 'static> RpcClient for JsonRpcClient<T> {
    async fn call<P, R>(&self, method: &str, params: P) -> Result<R, RpcError>
    where
        P: Serialize + Send,
        R: DeserializeOwned,
    {
        let id = Self::generate_id();

        let params_value = serde_json::to_value(params)?;
        let params_opt = if params_value.is_null() {
            None
        } else {
            Some(params_value)
        };

        let request = JsonRpcRequest::new(id.clone(), method, params_opt);
        let request_json = serde_json::to_string(&request)?;

        let (tx, rx) = oneshot::channel();

        {
            let mut pending = self.pending_requests.lock().await;
            pending.insert(id.clone(), tx);
        }

        self.transport.send(request_json.as_bytes()).await?;

        let response = tokio::time::timeout(self.timeout, rx)
            .await
            .map_err(|_| {
                let pending = self.pending_requests.clone();
                let id = id.clone();
                tokio::spawn(async move {
                    pending.lock().await.remove(&id);
                });
                RpcError::Timeout
            })?
            .map_err(|_| RpcError::ConnectionClosed)?;

        if let Some(err) = response.error {
            return Err(err.into());
        }

        let result = response
            .result
            .ok_or_else(|| RpcError::InvalidResponse("Missing result in response".into()))?;

        serde_json::from_value(result).map_err(|e| {
            RpcError::InvalidResponse(format!("Failed to deserialize result: {}", e))
        })
    }

    fn notifications(&self) -> broadcast::Receiver<JsonRpcNotification> {
        self.notification_sender.subscribe()
    }
}
