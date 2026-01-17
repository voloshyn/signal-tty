mod infrastructure;

use infrastructure::{SignalClient, SignalRepository};

fn parse_account() -> Option<String> {
    let args: Vec<String> = std::env::args().collect();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "-a" && i + 1 < args.len() {
            return Some(args[i + 1].clone());
        }
        i += 1;
    }
    None
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let account = parse_account();
    let client = SignalClient::new(account);

    println!("Connecting to signal-cli...");
    client.connect().await?;
    println!("Connected! Waiting for a message...");

    let mut messages = client.incoming_messages();

    for _ in 0..20 {
        if let Ok(msg) = messages.recv().await {
            println!("Received message from: {}", msg.envelope.sender_display());
            if let Some(data) = &msg.envelope.data_message {
                if let Some(text) = &data.message {
                    println!("Text: {}", text);
                    break;
                }
            } else if let Some(sync) = &msg.envelope.sync_message {
                if let Some(sent) = &sync.sent_message {
                    if let Some(text) = &sent.message {
                        println!("Sent to: {:?}", sent.destination);
                        println!("Text: {}", text);
                        break;
                    }
                }
                println!("(Sync message)");
            } else if msg.envelope.receipt_message.is_some() {
                println!("(Receipt/delivery confirmation)");
            } else if msg.envelope.typing_message.is_some() {
                println!("(Typing indicator)");
            }
        }
    }

    client.disconnect().await?;
    Ok(())
}
