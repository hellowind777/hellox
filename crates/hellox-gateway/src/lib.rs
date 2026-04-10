mod error;
mod files;
mod metrics;
mod streaming;

use std::collections::{BTreeMap, HashMap};
use std::convert::Infallible;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use axum::extract::State;
use axum::http::header;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use hellox_config::{load_or_default, HelloxConfig, ProviderConfig};
use hellox_core::{AnthropicCompatAdapter, ModelProfile};
use hellox_gateway_api::{
    AnthropicCompatRequest, AnthropicCompatResponse, ModelCard, ModelsResponse,
};
use hellox_provider_anthropic::AnthropicAdapter;
use hellox_provider_openai_compatible::OpenAiCompatibleAdapter;
use serde_json::{json, Value};
use tokio_stream::iter;
use tracing::info;

use crate::error::GatewayHttpError;
use crate::files::{files_upload, materialize_local_file_references};
use crate::metrics::GatewayMetrics;
use crate::streaming::anthropic_sse_events;

#[derive(Clone)]
pub struct GatewayState {
    pub config: HelloxConfig,
    pub profiles: BTreeMap<String, ModelProfile>,
    pub adapters: HashMap<String, Arc<dyn AnthropicCompatAdapter>>,
    pub metrics: Arc<GatewayMetrics>,
}

pub async fn serve(config_path: Option<PathBuf>) -> Result<()> {
    let state = build_state(load_or_default(config_path)?)?;
    let listen = state.config.gateway.listen.clone();

    let app = Router::new()
        .route("/health", get(health))
        .route("/metrics", get(metrics))
        .route("/v1/models", get(list_models))
        .route("/v1/messages", post(messages))
        .route("/v1/messages/stream", post(messages_stream))
        .route("/v1/files", post(files_upload))
        .route("/gateway/providers", get(list_providers))
        .route("/gateway/profiles", get(list_profiles))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&listen).await?;
    info!("hellox gateway listening on {}", listen);
    axum::serve(listener, app).await?;
    Ok(())
}

pub fn build_state(config: HelloxConfig) -> Result<GatewayState> {
    let profiles = hellox_config::materialize_profiles(&config);
    let mut adapters: HashMap<String, Arc<dyn AnthropicCompatAdapter>> = HashMap::new();

    for (name, provider) in &config.providers {
        let adapter: Arc<dyn AnthropicCompatAdapter> = match provider {
            ProviderConfig::Anthropic { .. } => Arc::new(AnthropicAdapter::from_config(provider)?),
            ProviderConfig::OpenAiCompatible { .. } => {
                Arc::new(OpenAiCompatibleAdapter::from_config(provider)?)
            }
        };
        adapters.insert(name.clone(), adapter);
    }

    Ok(GatewayState {
        config,
        profiles,
        adapters,
        metrics: Arc::new(GatewayMetrics::default()),
    })
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

async fn metrics(State(state): State<GatewayState>) -> impl IntoResponse {
    let body = state.metrics.render_prometheus();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        body,
    )
}

async fn list_models(State(state): State<GatewayState>) -> Json<ModelsResponse> {
    Json(ModelsResponse {
        data: state
            .profiles
            .values()
            .map(model_card_from_profile)
            .collect(),
    })
}

async fn list_providers(State(state): State<GatewayState>) -> Json<Value> {
    Json(serde_json::to_value(&state.config.providers).unwrap_or_else(|_| json!({})))
}

async fn list_profiles(State(state): State<GatewayState>) -> Json<Value> {
    Json(serde_json::to_value(&state.profiles).unwrap_or_else(|_| json!({})))
}

async fn messages(
    State(state): State<GatewayState>,
    Json(request): Json<AnthropicCompatRequest>,
) -> Result<Json<AnthropicCompatResponse>, GatewayHttpError> {
    let response = complete_request(&state, request).await?;
    Ok(Json(response))
}

async fn messages_stream(
    State(state): State<GatewayState>,
    Json(request): Json<AnthropicCompatRequest>,
) -> Result<Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>>, GatewayHttpError> {
    let response = complete_request(&state, request).await?;
    let events = anthropic_sse_events(&response);
    let stream = iter(events.into_iter().map(Ok::<Event, Infallible>));

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

async fn complete_request(
    state: &GatewayState,
    request: AnthropicCompatRequest,
) -> Result<AnthropicCompatResponse, GatewayHttpError> {
    let started = Instant::now();
    let requested_model = request.model.clone();
    let result = complete_request_inner(state, request).await;
    state
        .metrics
        .observe_request(started.elapsed(), result.is_ok());

    let mut response = result?;
    response.model = requested_model;
    Ok(response)
}

async fn complete_request_inner(
    state: &GatewayState,
    request: AnthropicCompatRequest,
) -> Result<AnthropicCompatResponse, GatewayHttpError> {
    let requested_model = request.model.clone();
    let profile = resolve_profile(&state.profiles, &requested_model).ok_or_else(|| {
        GatewayHttpError::bad_request(format!("unknown model profile: {requested_model}"))
    })?;
    let adapter = state.adapters.get(&profile.provider).ok_or_else(|| {
        GatewayHttpError::internal(format!(
            "provider adapter not registered: {}",
            profile.provider
        ))
    })?;

    let allow_unresolved_file_source = profile.provider == "anthropic";
    let mut upstream_request =
        materialize_local_file_references(request, allow_unresolved_file_source)?;
    upstream_request.model = profile.upstream_model.clone();
    upstream_request.stream = Some(false);

    adapter
        .complete(upstream_request)
        .await
        .map_err(GatewayHttpError::from)
}

fn resolve_profile<'a>(
    profiles: &'a BTreeMap<String, ModelProfile>,
    model: &str,
) -> Option<&'a ModelProfile> {
    profiles.get(model).or_else(|| {
        profiles
            .values()
            .find(|profile| profile.upstream_model == model)
    })
}

fn model_card_from_profile(profile: &ModelProfile) -> ModelCard {
    let mut capabilities = vec!["messages".to_string()];
    if profile.capabilities.tools {
        capabilities.push("tools".to_string());
    }
    if profile.capabilities.streaming {
        capabilities.push("streaming".to_string());
    }

    ModelCard {
        id: profile.name.clone(),
        display_name: Some(profile.display_name.clone()),
        provider: Some(profile.provider.clone()),
        capabilities,
    }
}
