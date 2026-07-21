use std::sync::Arc;

use axum::{Router, routing::post};

pub mod driver;
mod error;
pub mod fcm_client;
mod routes;

use crate::fcm_client::Fcm;

#[derive(Clone)]
struct AppState {
    pub db: Arc<dyn driver::Driver>,
    pub fcm: Arc<dyn Fcm>,
}

pub async fn build(db: Arc<dyn driver::Driver>, fcm: Arc<dyn Fcm>) -> anyhow::Result<Router> {
    fcm.validate().await?;
    tracing::info!("FCM credentials validated successfully");

    let state = AppState { db, fcm };

    let router = Router::new()
        .route(
            "/fcm-tokens/register",
            post(routes::register_fcm_token::register_fcm_token),
        )
        .route(
            "/topic-subscriptions/add",
            post(routes::add_topic_subscriptions::add_topic_subscriptions),
        )
        .route(
            "/topic-subscriptions/remove",
            post(routes::remove_topic_subscriptions::remove_topic_subscriptions),
        )
        .route(
            "/topic-subscriptions/update",
            post(routes::update_topic_subscriptions::update_topic_subscriptions),
        )
        .route(
            "/fcm-tokens/unregister",
            post(routes::unregister_fcm_token::unregister_fcm_token),
        )
        .route("/notify-topic", post(routes::notify_topic::notify_topics))
        .route("/report", post(routes::report::report))
        .with_state(state);

    Ok(router)
}
