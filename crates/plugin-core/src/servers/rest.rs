use orkester_common::messaging::{Message, ServerSide};
use orkester_common::plugin::servers::{Server, ServerError, ServerBuilder};
use serde_json::Value;

pub struct AxumRestServer {
    config: Value,
}

impl Server for AxumRestServer {
    fn start(&self, channel: ServerSide) -> Result<(), ServerError> {
        std::thread::spawn(move || {
            println!("[rest] Server started, listening for messages.");
            loop {
                match channel.from_hub.recv() {
                    Ok(msg) => match msg.message_type.as_str() {
                        "register_route" => {
                            let method = msg.content.get("method")
                                .and_then(|v| v.as_str()).unwrap_or("?");
                            let path = msg.content.get("path")
                                .and_then(|v| v.as_str()).unwrap_or("?");
                            println!(
                                "[rest] Registering route {} {} (requested by '{}').",
                                method, path, msg.source
                            );
                            let reply = Message::new(
                                0,    // hub-generated ack; sender tracks by original id in content
                                "",   // hub stamps the real source
                                msg.source.as_str(),
                                "route_registered",
                                serde_json::json!({
                                    "status": "ok",
                                    "method": method,
                                    "path": path,
                                }),
                            );
                            if channel.to_hub.send(reply).is_err() {
                                println!("[rest] Hub channel closed — stopping.");
                                return;
                            }
                        }
                        other => {
                            println!("[rest] Unhandled message type: '{}'.", other);
                        }
                    },
                    Err(_) => {
                        println!("[rest] Hub channel disconnected — stopping.");
                        break;
                    }
                }
            }
        });
        Ok(())
    }

    fn stop(&self) -> Result<(), ServerError> {
        Ok(())
    }
}

pub struct AxumRestServerBuilder;

impl ServerBuilder for AxumRestServerBuilder {
    fn build(&self, config: Value) -> Result<Box<dyn Server>, ServerError> {
        Ok(Box::new(AxumRestServer { config }))
    }
}
