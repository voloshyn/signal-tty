use super::error::SignalError;
use super::repository::SignalRepository;
use super::types::*;
use crate::infrastructure::jsonrpc::{JsonRpcClient, JsonRpcNotification, RpcClient};
use crate::infrastructure::transport::StdioTransport;
use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info};

pub struct SignalClient {
    rpc: Arc<JsonRpcClient<StdioTransport>>,
    account: Option<String>,
    connected: AtomicBool,
    message_sender: broadcast::Sender<IncomingMessage>,
}

impl SignalClient {
    pub fn new(account: Option<String>) -> Self {
        let transport = StdioTransport::new(account.clone());
        let rpc = Arc::new(JsonRpcClient::new(transport));
        let (message_sender, _) = broadcast::channel(256);

        Self {
            rpc,
            account,
            connected: AtomicBool::new(false),
            message_sender,
        }
    }

    fn spawn_notification_handler(&self) {
        let mut notifications = self.rpc.notifications();
        let message_sender = self.message_sender.clone();

        tokio::spawn(async move {
            loop {
                match notifications.recv().await {
                    Ok(notification) => {
                        if let Some(message) = Self::parse_notification(notification) {
                            if message_sender.send(message).is_err() {}
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        debug!("Notification receiver lagged by {} messages", n);
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        info!("Notification channel closed");
                        break;
                    }
                }
            }
        });
    }

    fn parse_notification(notification: JsonRpcNotification) -> Option<IncomingMessage> {
        if notification.method == "receive" {
            if let Some(params) = notification.params {
                match serde_json::from_value::<IncomingMessage>(params) {
                    Ok(msg) => return Some(msg),
                    Err(e) => {
                        error!("Failed to parse incoming message: {}", e);
                    }
                }
            }
        }
        None
    }

    async fn call<P, R>(&self, method: &str, params: P) -> Result<R, SignalError>
    where
        P: Serialize + Send,
        R: serde::de::DeserializeOwned,
    {
        let params_value = serde_json::to_value(params)?;
        self.rpc.call(method, params_value).await.map_err(|e| e.into())
    }
}

#[derive(Debug, Clone, Serialize, Default)]
struct EmptyParams {}

#[async_trait]
impl SignalRepository for SignalClient {
    async fn connect(&self) -> Result<(), SignalError> {
        self.rpc.connect().await?;
        self.connected.store(true, Ordering::SeqCst);
        self.spawn_notification_handler();
        info!("Connected to signal-cli daemon");
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    async fn disconnect(&self) -> Result<(), SignalError> {
        self.connected.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn get_account_info(&self) -> Result<Account, SignalError> {
        match &self.account {
            Some(number) => Ok(Account {
                number: number.clone(),
                uuid: None,
                device_id: None,
            }),
            None => Err(SignalError::NotRegistered),
        }
    }

    async fn list_accounts(&self) -> Result<Vec<String>, SignalError> {
        self.call("listAccounts", EmptyParams::default()).await
    }

    async fn send_message(&self, recipient: &str, message: &str) -> Result<SendResult, SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: Vec<String>,
            message: String,
        }

        self.call(
            "send",
            Params {
                recipient: vec![recipient.to_string()],
                message: message.to_string(),
            },
        )
        .await
    }

    async fn send_group_message(&self, group_id: &str, message: &str) -> Result<SendResult, SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            group_id: String,
            message: String,
        }

        self.call(
            "send",
            Params {
                group_id: group_id.to_string(),
                message: message.to_string(),
            },
        )
        .await
    }

    async fn send_message_with_attachments(&self, recipient: &str, message: &str, attachments: Vec<String>) -> Result<SendResult, SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: Vec<String>,
            message: String,
            attachments: Vec<String>,
        }

        self.call(
            "send",
            Params {
                recipient: vec![recipient.to_string()],
                message: message.to_string(),
                attachments,
            },
        )
        .await
    }

    async fn send_reaction(&self, recipient: &str, emoji: &str, target_author: &str, target_timestamp: i64) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            recipient: Vec<String>,
            emoji: String,
            target_author: String,
            target_timestamp: i64,
        }

        let _: Value = self
            .call(
                "sendReaction",
                Params {
                    recipient: vec![recipient.to_string()],
                    emoji: emoji.to_string(),
                    target_author: target_author.to_string(),
                    target_timestamp,
                },
            )
            .await?;
        Ok(())
    }

    async fn remove_reaction(&self, recipient: &str, emoji: &str, target_author: &str, target_timestamp: i64) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            recipient: Vec<String>,
            emoji: String,
            target_author: String,
            target_timestamp: i64,
            remove: bool,
        }

        let _: Value = self
            .call(
                "sendReaction",
                Params {
                    recipient: vec![recipient.to_string()],
                    emoji: emoji.to_string(),
                    target_author: target_author.to_string(),
                    target_timestamp,
                    remove: true,
                },
            )
            .await?;
        Ok(())
    }

    async fn list_contacts(&self) -> Result<Vec<Contact>, SignalError> {
        self.call("listContacts", EmptyParams::default()).await
    }

    async fn get_contact(&self, identifier: &str) -> Result<Contact, SignalError> {
        let contacts = self.list_contacts().await?;
        contacts
            .into_iter()
            .find(|c| c.number.as_deref() == Some(identifier) || c.uuid.as_deref() == Some(identifier))
            .ok_or_else(|| SignalError::ContactNotFound(identifier.to_string()))
    }

    async fn update_contact_name(&self, identifier: &str, name: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: String,
            name: String,
        }

        let _: Value = self
            .call(
                "updateContact",
                Params {
                    recipient: identifier.to_string(),
                    name: name.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    async fn block_contact(&self, identifier: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: Vec<String>,
        }

        let _: Value = self
            .call(
                "block",
                Params {
                    recipient: vec![identifier.to_string()],
                },
            )
            .await?;
        Ok(())
    }

    async fn unblock_contact(&self, identifier: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: Vec<String>,
        }

        let _: Value = self
            .call(
                "unblock",
                Params {
                    recipient: vec![identifier.to_string()],
                },
            )
            .await?;
        Ok(())
    }

    async fn list_groups(&self) -> Result<Vec<Group>, SignalError> {
        self.call("listGroups", EmptyParams::default()).await
    }

    async fn get_group(&self, group_id: &str) -> Result<Group, SignalError> {
        let groups = self.list_groups().await?;
        groups
            .into_iter()
            .find(|g| g.id == group_id)
            .ok_or_else(|| SignalError::GroupNotFound(group_id.to_string()))
    }

    async fn leave_group(&self, group_id: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            group_id: String,
        }

        let _: Value = self
            .call(
                "quitGroup",
                Params {
                    group_id: group_id.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    async fn block_group(&self, group_id: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            group_id: String,
        }

        let _: Value = self
            .call(
                "block",
                Params {
                    group_id: group_id.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    async fn list_identities(&self) -> Result<Vec<Identity>, SignalError> {
        self.call("listIdentities", EmptyParams::default()).await
    }

    async fn trust_identity(&self, identifier: &str, trust_all_keys: bool) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            recipient: String,
            trust_all_known_keys: bool,
        }

        let _: Value = self
            .call(
                "trust",
                Params {
                    recipient: identifier.to_string(),
                    trust_all_known_keys: trust_all_keys,
                },
            )
            .await?;
        Ok(())
    }

    async fn send_typing_started(&self, recipient: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: String,
        }

        let _: Value = self
            .call(
                "sendTyping",
                Params {
                    recipient: recipient.to_string(),
                },
            )
            .await?;
        Ok(())
    }

    async fn send_typing_stopped(&self, recipient: &str) -> Result<(), SignalError> {
        #[derive(Serialize)]
        struct Params {
            recipient: String,
            stop: bool,
        }

        let _: Value = self
            .call(
                "sendTyping",
                Params {
                    recipient: recipient.to_string(),
                    stop: true,
                },
            )
            .await?;
        Ok(())
    }

    async fn send_read_receipt(&self, recipient: &str, timestamps: Vec<i64>) -> Result<(), SignalError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Params {
            recipient: String,
            target_timestamp: Vec<i64>,
        }

        let _: Value = self
            .call(
                "sendReceipt",
                Params {
                    recipient: recipient.to_string(),
                    target_timestamp: timestamps,
                },
            )
            .await?;
        Ok(())
    }

    fn incoming_messages(&self) -> broadcast::Receiver<IncomingMessage> {
        self.message_sender.subscribe()
    }
}

impl From<serde_json::Error> for SignalError {
    fn from(e: serde_json::Error) -> Self {
        SignalError::Rpc(crate::infrastructure::jsonrpc::RpcError::Serialization(e))
    }
}
