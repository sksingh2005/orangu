// Copyright (C) 2026 The orangu community
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use crate::*;

#[derive(Debug, Deserialize)]
pub(crate) struct ModelsResponse {
    #[serde(default)]
    data: Vec<ModelEntry>,
    #[serde(default)]
    models: Vec<ModelEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ModelEntry {
    #[serde(default)]
    id: String,
    #[serde(default)]
    model: String,
    #[serde(default)]
    name: String,
}

/// Build a GET request to a server's `/v1/models` endpoint, attaching the
/// optional bearer token. OpenAI-compatible servers — including a llama.cpp
/// server started with `--api-key` — require `Authorization: Bearer <key>` on
/// every `/v1/*` endpoint, not just chat completions.
pub(crate) fn models_request(
    http_client: &reqwest::Client,
    endpoint: &str,
    api_key: Option<&str>,
) -> reqwest::RequestBuilder {
    let url = format!("{}/v1/models", normalized_openai_endpoint(endpoint));
    let request = http_client.get(url);
    match api_key {
        Some(key) => request.bearer_auth(key),
        None => request,
    }
}

/// Probe the active server and return its header status together with the list
/// of model ids it advertises (used for `/model` completion). `model_ok` is set
/// when the active wire model id is among the advertised models.
pub(crate) async fn probe_header_status(
    http_client: &reqwest::Client,
    workspace: &Path,
    active_model_id: &str,
    profile: &LlmConfiguration,
    endpoint: Option<&str>,
) -> (orangu::tui::HeaderStatus, Vec<String>) {
    let workspace_ok = workspace.exists();
    let mut server_ok = false;
    let mut model_ok = false;
    let mut available_models = Vec::new();

    if let Some(endpoint) = endpoint
        && let Ok(response) = models_request(http_client, endpoint, profile.api_key.as_deref())
            .send()
            .await
        && response.status().is_success()
    {
        server_ok = true;
        if let Ok(models) = response.json::<ModelsResponse>().await {
            for entry in models.data.iter().chain(models.models.iter()) {
                let id = if !entry.id.is_empty() {
                    &entry.id
                } else if !entry.model.is_empty() {
                    &entry.model
                } else if !entry.name.is_empty() {
                    &entry.name
                } else {
                    continue;
                };
                if id == active_model_id
                    || entry.model == active_model_id
                    || entry.name == active_model_id
                {
                    model_ok = true;
                }
                available_models.push(id.clone());
            }
        }
    }

    (
        orangu::tui::HeaderStatus {
            workspace_ok,
            server_ok,
            model_ok,
        },
        available_models,
    )
}

/// If the active server is not serving the configured model at startup, switch
/// to a model the server actually advertises. Returns `(old, new)` model ids
/// when a switch happened. The server (endpoint, provider, system prompt) is
/// unchanged — only the wire model id moves.
pub(crate) async fn try_startup_model_switch(
    http_client: &reqwest::Client,
    profile: &LlmConfiguration,
    active_model_id: &mut String,
    endpoint: Option<&str>,
) -> Option<(String, String)> {
    let endpoint = endpoint?;
    let response = models_request(http_client, endpoint, profile.api_key.as_deref())
        .send()
        .await
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let models = response.json::<ModelsResponse>().await.ok()?;

    let available: Vec<String> = models
        .data
        .iter()
        .chain(models.models.iter())
        .filter_map(|entry| {
            if !entry.id.is_empty() {
                Some(entry.id.clone())
            } else if !entry.model.is_empty() {
                Some(entry.model.clone())
            } else if !entry.name.is_empty() {
                Some(entry.name.clone())
            } else {
                None
            }
        })
        .collect();

    // The server already serves the configured model — nothing to switch.
    if available.iter().any(|model| model == active_model_id) {
        return None;
    }

    // Otherwise move to the first model the server actually offers.
    let new_model = available.into_iter().next()?;
    let old = std::mem::replace(active_model_id, new_model.clone());
    Some((old, new_model))
}

#[cfg(test)]
mod tests {

    #[test]
    fn models_request_attaches_optional_bearer_token() {
        let client = reqwest::Client::new();

        let with_key = super::models_request(&client, "http://localhost:8100/v1", Some("secret"))
            .build()
            .expect("build request");
        assert_eq!(with_key.url().as_str(), "http://localhost:8100/v1/models");
        assert_eq!(
            with_key
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .and_then(|value| value.to_str().ok()),
            Some("Bearer secret")
        );

        let without_key = super::models_request(&client, "http://localhost:8100/v1", None)
            .build()
            .expect("build request");
        assert!(
            without_key
                .headers()
                .get(reqwest::header::AUTHORIZATION)
                .is_none()
        );
    }
}
