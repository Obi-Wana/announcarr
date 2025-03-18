use reqwest::Client;
use serde::{Deserialize};
use tracing::{debug, info, error};

#[derive(Debug, Deserialize)]
struct ApiResponse {
    data: Vec<ApiItem>
}

#[derive(Debug, Deserialize)]
pub struct ApiItem {
    pub id: String,
    pub attributes: Attributes,
}

#[derive(Debug, Deserialize)]
pub struct Attributes {
    pub category: String,
    pub r#type: String,
    pub name: String,
    pub resolution: Option<String>,
    pub freeleech: String,
    pub internal: u8,
    pub double_upload: bool,
    pub size: u64,
    pub uploader: String,
    pub download_link: String,
    pub bumped_at: String,
}

pub struct ApiHandler {
    client: Client,
    url: String,
    token: String,
}

impl ApiHandler {
    pub fn new(url: String, token: String) -> Self {
        Self {
            client: Client::new(),
            url,
            token,
        }
    }

    pub async fn fetch_messages(&self) -> Vec<ApiItem> {
        info!("⬇️ Fetching API {} ...", &self.url);

        match self.client.get(&self.url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await
        {
            Ok(response) => {
                let body = match response.text().await {
                    Ok(text) => text,
                    Err(e) => {
                        error!("Failed to read response body: {}", e);
                        return vec![];
                    }
                };

                debug!("Full API response body: {}", body);

                match serde_json::from_str::<ApiResponse>(&body) {
                    Ok(api_response) => api_response.data,
                    Err(e) => {
                        error!("Failed to parse API response: {}", e);
                        vec![]
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch messages from API: {}", e);
                vec![]
            }
        }
    }
}
