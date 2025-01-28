use log::{error, info, warn};
use reqwest::Client;
use serde_derive::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use sysinfo::{System, SystemExt};
use tokio::sync::Mutex;
use tokio::time::interval;

pub struct AnalyticsManager {
    client: Client,
    posthog_api_key: String,
    distinct_id: String,
    interval: Duration,
    enabled: Arc<Mutex<bool>>,
    api_host: String,
    local_api_base_url: String,
}

impl AnalyticsManager {
    pub fn new(
        posthog_api_key: String,
        distinct_id: String,
        interval_hours: u64,
        local_api_base_url: String,
    ) -> Self {
        Self {
            client: Client::new(),
            posthog_api_key,
            distinct_id,
            interval: Duration::from_secs(interval_hours * 3600),
            enabled: Arc::new(Mutex::new(!cfg!(debug_assertions))),
            api_host: "https://eu.i.posthog.com".to_string(),
            local_api_base_url,
        }
    }

    pub async fn send_event(
        &self,
        event: &str,
        properties: Option<serde_json::Value>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !*self.enabled.lock().await {
            return Ok(());
        }

        let posthog_url = format!("{}/capture/", self.api_host);
        let system = System::new_all();

        let mut payload = json!({
            "api_key": self.posthog_api_key,
            "event": event,
            "properties": {
                "distinct_id": self.distinct_id,
                "$lib": "rust-reqwest",
                "os_name": system.name().unwrap_or_default(),
                "os_version": system.os_version().unwrap_or_default(),
                "kernel_version": system.kernel_version().unwrap_or_default(),
                "host_name": system.host_name().unwrap_or_default(),
                "cpu_count": system.cpus().len(),
                "total_memory": system.total_memory(),
            },
        });

        if let Some(props) = properties {
            if let Some(payload_props) = payload["properties"].as_object_mut() {
                payload_props.extend(props.as_object().unwrap_or(&serde_json::Map::new()).clone());
            }
        }

        let response = self.client.post(posthog_url).json(&payload).send().await?;

        if !response.status().is_success() {
            return Err(format!("PostHog API error: {}", response.status()).into());
        }

        Ok(())
    }

    pub async fn start_periodic_event(&self) {
        let mut interval = interval(self.interval);

        loop {
            interval.tick().await;
            if *self.enabled.lock().await {
                // Get health status
                let health_status = match self.check_recording_health().await {
                    Ok(status) => status,
                    Err(e) => {
                        error!("failed to check recording health: {}", e);
                        json!({
                            "is_healthy": false,
                            "frame_status": "error",
                            "audio_status": "error",
                            "ui_status": "error",
                            "error": e.to_string()
                        })
                    }
                };

                // Send periodic event with health data
                if let Err(e) = self.send_event("app_still_running", Some(health_status)).await {
                    error!("failed to send periodic posthog event: {}", e);
                }

                // Track enabled pipes
                if let Err(e) = self.track_enabled_pipes().await {
                    warn!("failed to track enabled pipes: {}, is screenpipe up?", e);
                }
            }
        }
    }

    async fn check_recording_health(&self) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        let health_url = format!("{}/health", self.local_api_base_url);
        let response = self.client.get(&health_url).send().await?;
        
        if !response.status().is_success() {
            return Ok(json!({
                "is_healthy": false,
                "frame_status": "error",
                "audio_status": "error",
                "ui_status": "error",
                "error": format!("Health check failed with status: {}", response.status())
            }));
        }

        let health: serde_json::Value = response.json().await?;
        
        // Extract relevant status fields
        let frame_status = health["frame_status"].as_str().unwrap_or("unknown");
        let audio_status = health["audio_status"].as_str().unwrap_or("unknown");
        let ui_status = health["ui_status"].as_str().unwrap_or("unknown");
        
        // Consider healthy if all enabled systems are "ok"
        let is_healthy = (frame_status == "ok" || frame_status == "disabled") &&
                        (audio_status == "ok" || audio_status == "disabled") &&
                        (ui_status == "ok" || ui_status == "disabled");

        Ok(json!({
            "is_healthy": is_healthy,
            "frame_status": frame_status,
            "audio_status": audio_status,
            "ui_status": ui_status
        }))
    }

    async fn track_enabled_pipes(&self) -> Result<(), Box<dyn std::error::Error>> {
        let pipes_url = format!("{}/pipes/list", self.local_api_base_url);
        let response: PipeListResponse = self.client.get(&pipes_url).send().await?.json().await?;

        let enabled_pipes: Vec<String> = response
            .data
            .into_iter()
            .filter(|pipe| pipe.enabled)
            .map(|pipe| pipe.id)
            .collect();

        let properties = json!({
            "enabled_pipes": enabled_pipes,
            "enabled_pipe_count": enabled_pipes.len(),
        });

        self.send_event("enabled_pipes_hourly", Some(properties))
            .await
    }
}

pub fn start_analytics(
    unique_id: String,
    posthog_api_key: String,
    interval_hours: u64,
    local_api_base_url: String,
) -> Result<Arc<AnalyticsManager>, Box<dyn std::error::Error>> {
    let is_debug = std::env::var("TAURI_ENV_DEBUG").unwrap_or("false".to_string()) == "true";
    if cfg!(debug_assertions) || is_debug {
        info!("skipping analytics in development mode");
        return Ok(Arc::new(AnalyticsManager::new(
            posthog_api_key,
            unique_id,
            interval_hours,
            local_api_base_url,
        )));
    }

    let analytics_manager = Arc::new(AnalyticsManager::new(
        posthog_api_key,
        unique_id,
        interval_hours,
        local_api_base_url,
    ));

    // Send initial event at boot
    tokio::spawn({
        let analytics_manager = analytics_manager.clone();
        async move {
            if let Err(e) = analytics_manager.send_event("app_started", None).await {
                error!("Failed to send initial PostHog event: {}", e);
            }
        }
    });

    // Start periodic events
    tokio::spawn({
        let analytics_manager = analytics_manager.clone();
        async move {
            analytics_manager.start_periodic_event().await;
        }
    });

    Ok(analytics_manager)
}

#[derive(Deserialize)]
struct PipeInfo {
    id: String,
    enabled: bool,
}

#[derive(Deserialize)]
struct PipeListResponse {
    data: Vec<PipeInfo>,
    #[allow(dead_code)]
    success: bool,
}
