use std::sync::Arc;

use axum::{Router, routing::post};

pub mod client;
pub mod driver;
mod error;
pub mod fcm_client;
pub mod routes;
pub mod types;

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
            "/register-fcm-token",
            post(routes::register_fcm_token::register_fcm_token),
        )
        .route("/subscribe", post(routes::subscribe::subscribe))
        .route("/unsubscribe", post(routes::unsubscribe::unsubscribe))
        .route(
            "/set-subscriptions",
            post(routes::set_subscriptions::set_subscriptions),
        )
        .route("/notify-topic", post(routes::notify_topic::notify_topics))
        .with_state(state);

    Ok(router)
}
