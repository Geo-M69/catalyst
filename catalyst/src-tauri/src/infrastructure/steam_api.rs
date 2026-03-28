#![allow(dead_code)]

use reqwest::blocking::Client;
use serde_json::Value;

use crate::application::error::{AppError, AppResult};
use crate::application::ports::effects::HttpPort;

pub(crate) struct SteamApi {
    client: Client,
}

impl SteamApi {
    pub(crate) fn new() -> AppResult<Self> {
        let client = crate::build_http_client().map_err(AppError::from)?;
        Ok(Self { client })
    }

    pub(crate) fn get_text(&self, url: &str) -> AppResult<String> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| AppError::from(format!("HTTP GET failed: {error}")))?;

        if !response.status().is_success() {
            return Err(AppError::from(format!(
                "HTTP GET failed with status {}",
                response.status()
            )));
        }

        response
            .text()
            .map_err(|error| AppError::from(format!("Failed to read HTTP body: {error}")))
    }

    pub(crate) fn get_json(&self, url: &str) -> AppResult<Value> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| AppError::from(format!("HTTP GET failed: {error}")))?;

        if !response.status().is_success() {
            return Err(AppError::from(format!(
                "HTTP GET failed with status {}",
                response.status()
            )));
        }

        response
            .json::<Value>()
            .map_err(|error| AppError::from(format!("Failed to decode JSON body: {error}")))
    }

    pub(crate) fn post_form_text(&self, url: &str, body: &[(&str, &str)]) -> AppResult<String> {
        let response = self
            .client
            .post(url)
            .form(body)
            .send()
            .map_err(|error| AppError::from(format!("HTTP POST failed: {error}")))?;

        if !response.status().is_success() {
            return Err(AppError::from(format!(
                "HTTP POST failed with status {}",
                response.status()
            )));
        }

        response
            .text()
            .map_err(|error| AppError::from(format!("Failed to read HTTP body: {error}")))
    }
}

impl HttpPort for SteamApi {
    fn get_text(&self, endpoint: &str) -> AppResult<String> {
        SteamApi::get_text(self, endpoint)
    }

    fn post_form_text(&self, endpoint: &str, body: &[(&str, &str)]) -> AppResult<String> {
        SteamApi::post_form_text(self, endpoint, body)
    }
}
