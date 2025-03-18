use std::error::Error;
use tracing::{debug, info};
use tokio_stream::StreamExt as _;
use tokio::time::{interval, Instant, Duration};

use web_api::ApiHandler;
use irc_client::IrcClient;
use config::{load_config};

mod config;
mod irc_client;
mod web_api;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Logging
    tracing_subscriber::fmt::init();
    //tracing_subscriber::fmt().with_max_level(tracing::Level::DEBUG).init();

    // Load the IRC configuration from the TOML file
    debug!("Loading configuration file ...");
    let config = load_config();

    // Initialize the IRC client
    debug!("Initialize IRC client ...");
    let mut irc_client = IrcClient::new(
        config.irc,
        config.app.announced_file,
    ).await?;
    irc_client.connect().await?;
    irc_client.verify_connected().await;

    // Initialize the API client
    debug!("Initialize API client ...");
    let api_handler = ApiHandler::new(
        config.api.url,
        config.api.token,
    );

    // Main loop to keep the bot connected and fetch/post messages
    info!("✅ Application started");
    let mut interval = interval(Duration::from_secs(2));
    let mut last_api_call = Instant::now() - Duration::from_secs(30);
    let mut connection_check = tokio::time::interval(Duration::from_secs(60));

    loop {
        tokio::select! {
            Some(message) = irc_client.stream.next() => {
                print!("{}", message?);
            }

            _ = interval.tick() => {
                let now = Instant::now();

                // Only fetch if the rate limit allows
                if now.duration_since(last_api_call) >= Duration::from_secs(30) {
                    let messages = api_handler.fetch_messages().await;

                    for message in messages {
                        if irc_client.should_announce(&message).await {
                            let _ = irc_client.send_message(message).await;
                        }
                    }
                    // Update last successful API call time
                    last_api_call = now;
                }
                else {
                    debug!("Skipping API call to avoid rate limit");
                }
            }

            // Connection verification (will crash on failure)
            _ = connection_check.tick() => {
                irc_client.verify_connected().await;
            }
        }
    }
}
