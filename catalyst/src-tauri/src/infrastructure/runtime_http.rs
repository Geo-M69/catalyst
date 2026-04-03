use reqwest::blocking::Client;
use std::time::Duration;

pub(crate) fn build_http_client() -> Result<Client, String> {
    Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .map_err(|error| format!("Failed to initialize HTTP client: {error}"))
}
