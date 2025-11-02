use irc::client::prelude::*;
use tokio_stream::StreamExt as _;
use tracing::{debug, info, warn, error};
use std::fs::{self, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;
use serde::{Serialize, Deserialize};
use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::config::IrcConfig;
use crate::web_api::ApiItem;

#[derive(Debug, Serialize, Deserialize, Clone, Hash, Eq, PartialEq)]
struct SeenItem {
    id: String,
    bumped_at: String,
}

pub struct IrcClient {
    pub client: Client,
    pub config: IrcConfig,
    pub stream: irc::client::ClientStream,
    seen_ids: Arc<Mutex<HashSet<SeenItem>>>,
    announced_file: String,
}

impl IrcClient {
    pub async fn new(config: IrcConfig, announced_file: String) -> irc::error::Result<Self> {
        let irc_config = Config {
            nickname: Some(config.nickname.to_string()),
            password: Some(config.password.to_string()),
            server: Some(config.server.to_owned()),
            port: Some(config.port),
            use_tls: Some(config.use_tls),
            channels: vec![config.channel.to_string()],
            ..Config::default()
        };

        let seen_ids = match Self::load_seen_ids(&announced_file) {
            Ok(ids) => ids,
            Err(e) => {
                error!("Failed to load seen IDs: {}", e);
                HashSet::new()
            }
        };

        let mut client = Client::from_config(irc_config).await?;
        let stream = client.stream()?;

        Ok(Self {
            client,
            stream,
            config,
            seen_ids: Arc::new(Mutex::new(seen_ids)),
            announced_file,
        })
    }

    pub async fn connect(&mut self) -> irc::error::Result<()> {
        self.client.identify()?;

        // Wait for successful registration (001 RPL_WELCOME) until nickserv auth
        info!("⏳ Waiting for server registration...");
        while let Some(message) = self.stream.next().await {
            let message = message?;

            // Check for successful registration
            if let Command::Response(Response::RPL_WELCOME, _) = message.command {
                info!("✅ Registered with server");
                break;
            }

            // Also respond to PING during registration
            if let Command::PING(server, _) = &message.command {
                self.client.send_pong(server)?;
            }
        }

        info!("🪪  NickServ identifying as {} ...", self.config.nickname);
        self.client.send_privmsg("NickServ", format!("IDENTIFY {} {}", self.config.nickname, self.config.ns_password))?;
        // Wait for the NickServ confirmation message
        info!("⏳ Waiting for NickServ confirmation...");
        while let Some(message) = self.stream.next().await {
            let message = message?;

            if let Command::NOTICE(target, content) = message.command {
                if content.contains("Password accepted") {
                    info!("✅ NickServ identification successful");
                    break;
                }
            }
        }

        info!("⏳ Joining {} ...", self.config.channel);
        self.client.send_join(self.config.channel.to_string())?;

        while let Some(message) = self.stream.next().await {
            let message = message?;

            if let Command::Response(_, ref text) = &message.command {
                if text.contains(&String::from("End of /NAMES list.")) {
                    info!("✅ Channel {} joined", self.config.channel);

                    // Now that we're fully connected, try OPER if needed
                    if let Some(true) = &self.config.oper {
                        info!("⏳ Attempting to gain operator privileges...");
                        self.client.send_oper(&self.config.nickname, &self.config.password)?;
                    }

                    return Ok(());
                }
            }
        }
        Ok(())
    }

    pub async fn verify_connected(&mut self) -> bool {
        debug!("Performing IRC connection check (Pong) ...");

        // Verify with a WHOIS/PING
        match self.client.send_pong(&self.config.nickname) {
            Ok(_) => {
                debug!("✅ IRC connection ok");
                true
            }
            Err(e) => {
                error!("❌ IRC connection check failed: {}", e);
                false
            }
        }
    }

    fn load_seen_ids(announced_file: &str) -> Result<HashSet<SeenItem>, Box<dyn std::error::Error>> {
        debug!("Load seen list from file ...");
        if !Path::new(announced_file).exists() {
            let file = OpenOptions::new()
                .write(true)
                .truncate(true)
                .open(announced_file)?;

            let mut writer = BufWriter::new(file);
            serde_json::to_writer(&mut writer, &Vec::<SeenItem>::new())?;
            writer.flush()?;

            return Ok(HashSet::new());
        }

        let file_content = fs::read_to_string(announced_file)?;
        let seen_items: Vec<SeenItem> = serde_json::from_str(&file_content)?;

        Ok(seen_items.into_iter().collect())
    }

    async fn save_seen_ids(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Saving ID & timestamp to file ...");

        // Acquire the lock only for the duration of cloning the seen IDs
        let seen_items: Vec<SeenItem> = {
            let seen = self.seen_ids.lock().await;
            seen.iter().cloned().collect()
        };

        debug!("Writing to file ...");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&self.announced_file)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, &seen_items)?;
        writer.flush()?;

        Ok(())
    }

    pub async fn should_announce(&self, item: &ApiItem) -> bool {
        let seen_item = SeenItem {
            id: item.id.clone(),
            bumped_at: item.attributes.bumped_at.clone(),
        };

        let mut seen = self.seen_ids.lock().await;

        // First check if we have an exact match (same ID and timestamp)
        if seen.contains(&seen_item) {
            debug!("⏭️ Already announced ID {}, skipping", seen_item.id);
            return false;
        }

        // If we get here, either:
        // 1. The item doesn't exist in the set, or
        // 2. It exists but with a different timestamp
        // So we remove any existing entry with the same ID (if present)
        seen.retain(|s| s.id != seen_item.id);
        true
    }

    pub async fn send_message(&mut self, item: ApiItem) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Format and announce the message
        let message = self.format_message(&item).await;

        // Try to send the message
        info!("📢 Announcing: {}", message);
        // Try to send message
        self.client.send_privmsg(&self.config.channel, &message)?;

        // Verify connected
        if !self.verify_connected().await {
            warn!("❌ Failed to announce ID {}, not connected to channel {}", &item.id, &self.config.channel);
            warn!("Will not store this ID to the log file");
            return Err("Message failed to send to channel".into());
        }

        debug!("✅ Message confirmed, marking item with ID {} as seen", &item.id);
        self.mark_as_announced(&item).await;
        Ok(())
    }

    pub async fn mark_as_announced(&self, item: &ApiItem) {
        let seen_item = SeenItem {
            id: item.id.clone(),
            bumped_at: item.attributes.bumped_at.clone(),
        };

        // Only hold lock for the insert operation
        {
            let mut seen = self.seen_ids.lock().await;
            seen.insert(seen_item);
        }
        
        if let Err(e) = self.save_seen_ids().await {
            error!("Failed to save seen IDs: {}", e);
        }
    }

    async fn format_message(&self, item: &crate::web_api::ApiItem) -> String {
        // Handle optional resolution
        let resolution = item.attributes.resolution.as_deref().unwrap_or("N/A");

        // Determine internal status
        let internal_status = match item.attributes.internal {
            0 => "No",
            1 => "Yes",
            _ => "N/A",
        };

        // Determine double upload status
        let du_status = if item.attributes.double_upload {
            "Yes"
        } else {
            "No"
        };

        // Convert Bytes to GB
        let size_in_gb = ((item.attributes.size as f64 / (1024.0 * 1024.0 * 1024.0)) * 100.0).round() / 100.0;

        // Extract the download link
        let download_link = item.attributes.download_link
            .replace("torrent", "torrents");
        let download_link = download_link.rsplit_once('.').map(|x| x.0)
            .unwrap_or("N/A");

        // Format the message
        format!(
            "Category [{}] Type [{}] Name [{}] Resolution [{}] Freeleech [{}] Internal [{}] Double Upload [{}] Size [{} GB] Uploader [{}] Url [{}]",
            item.attributes.category,
            item.attributes.r#type,
            item.attributes.name,
            resolution,
            item.attributes.freeleech,
            internal_status,
            du_status,
            size_in_gb,
            item.attributes.uploader,
            download_link,
        )
    }
}
