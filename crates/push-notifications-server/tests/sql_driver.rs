use std::collections::HashSet;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use push_notifications_client::types::{FcmToken, PublicKey, TopicId};
use push_notifications_server::driver::Driver;
use push_notifications_server::driver::sql::SqlDriver;

static INIT_DRIVERS: Once = Once::new();
static DB_COUNTER: AtomicU32 = AtomicU32::new(0);

async fn create_driver() -> SqlDriver {
    INIT_DRIVERS.call_once(|| sqlx::any::install_default_drivers());
    let id = DB_COUNTER.fetch_add(1, Ordering::Relaxed);
    let url = format!("sqlite:file:test_{id}?mode=memory&cache=shared");
    SqlDriver::new(&url).await.unwrap()
}

// --- FCM tokens ---

#[tokio::test]
async fn store_and_get_fcm_token() {
    let db = create_driver().await;

    db.store_fcm_token(
        &PublicKey::from("alice".to_string()),
        &FcmToken::from("tok-1".to_string()),
    )
    .await
    .unwrap();

    let result = db
        .get_fcm_token(&PublicKey::from("alice".to_string()))
        .await
        .unwrap();
    assert_eq!(result, Some(FcmToken::from("tok-1".to_string())));
}

#[tokio::test]
async fn get_fcm_token_missing() {
    let db = create_driver().await;

    let result = db
        .get_fcm_token(&PublicKey::from("nobody".to_string()))
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn store_fcm_token_overwrites() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.store_fcm_token(&alice, &FcmToken::from("tok-1".to_string()))
        .await
        .unwrap();
    db.store_fcm_token(&alice, &FcmToken::from("tok-2".to_string()))
        .await
        .unwrap();

    let result = db.get_fcm_token(&alice).await.unwrap();
    assert_eq!(result, Some(FcmToken::from("tok-2".to_string())));
}

// --- subscribe / get_subscribers ---

#[tokio::test]
async fn subscribe_and_get_subscribers() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.subscribe_to_topics(
        &alice,
        &["t1", "t2"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    let subs = db
        .get_subscribers(&TopicId::from("t1".to_string()))
        .await
        .unwrap();
    assert_eq!(subs, vec![alice.clone()]);

    let subs = db
        .get_subscribers(&TopicId::from("t2".to_string()))
        .await
        .unwrap();
    assert_eq!(subs, vec![alice]);
}

#[tokio::test]
async fn subscribe_is_idempotent() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());
    let t1: HashSet<TopicId> = [TopicId::from("t1".to_string())].into();

    db.subscribe_to_topics(&alice, &t1).await.unwrap();
    db.subscribe_to_topics(&alice, &t1).await.unwrap();

    let subs = db
        .get_subscribers(&TopicId::from("t1".to_string()))
        .await
        .unwrap();
    assert_eq!(subs, vec![alice]);
}

#[tokio::test]
async fn get_subscribers_empty_topic() {
    let db = create_driver().await;

    let subs = db
        .get_subscribers(&TopicId::from("nobody".to_string()))
        .await
        .unwrap();
    assert!(subs.is_empty());
}

#[tokio::test]
async fn multiple_subscribers_same_topic() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());
    let bob = PublicKey::from("bob".to_string());
    let t1: HashSet<TopicId> = [TopicId::from("t1".to_string())].into();

    db.subscribe_to_topics(&alice, &t1).await.unwrap();
    db.subscribe_to_topics(&bob, &t1).await.unwrap();

    let mut subs = db
        .get_subscribers(&TopicId::from("t1".to_string()))
        .await
        .unwrap();
    subs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    assert_eq!(subs, vec![alice, bob]);
}

// --- unsubscribe ---

#[tokio::test]
async fn unsubscribe_removes_subscription() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.subscribe_to_topics(
        &alice,
        &["t1", "t2"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    db.unsubscribe_from_topics(&alice, &[TopicId::from("t1".to_string())].into())
        .await
        .unwrap();

    let subs = db
        .get_subscribers(&TopicId::from("t1".to_string()))
        .await
        .unwrap();
    assert!(subs.is_empty());

    let subs = db
        .get_subscribers(&TopicId::from("t2".to_string()))
        .await
        .unwrap();
    assert_eq!(subs, vec![alice]);
}

#[tokio::test]
async fn unsubscribe_only_affects_target_user() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());
    let bob = PublicKey::from("bob".to_string());
    let t1: HashSet<TopicId> = [TopicId::from("t1".to_string())].into();

    db.subscribe_to_topics(&alice, &t1).await.unwrap();
    db.subscribe_to_topics(&bob, &t1).await.unwrap();

    db.unsubscribe_from_topics(&alice, &t1).await.unwrap();

    let subs = db
        .get_subscribers(&TopicId::from("t1".to_string()))
        .await
        .unwrap();
    assert_eq!(subs, vec![bob]);
}

// --- set_subscriptions ---

#[tokio::test]
async fn set_subscriptions_replaces_all() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.subscribe_to_topics(
        &alice,
        &["t1", "t2", "t3"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    db.set_subscriptions(
        &alice,
        &["t2", "t4"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    assert!(
        db.get_subscribers(&TopicId::from("t1".to_string()))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        db.get_subscribers(&TopicId::from("t3".to_string()))
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        db.get_subscribers(&TopicId::from("t2".to_string()))
            .await
            .unwrap(),
        vec![alice.clone()]
    );
    assert_eq!(
        db.get_subscribers(&TopicId::from("t4".to_string()))
            .await
            .unwrap(),
        vec![alice]
    );
}

#[tokio::test]
async fn set_subscriptions_empty_clears_all() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.subscribe_to_topics(
        &alice,
        &["t1", "t2"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    db.set_subscriptions(&alice, &HashSet::new()).await.unwrap();

    assert!(
        db.get_subscribers(&TopicId::from("t1".to_string()))
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        db.get_subscribers(&TopicId::from("t2".to_string()))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn set_subscriptions_does_not_affect_other_users() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());
    let bob = PublicKey::from("bob".to_string());
    let t1_t2: HashSet<TopicId> = ["t1", "t2"]
        .into_iter()
        .map(|s| TopicId::from(s.to_string()))
        .collect();

    db.subscribe_to_topics(&alice, &t1_t2).await.unwrap();
    db.subscribe_to_topics(&bob, &t1_t2).await.unwrap();

    db.set_subscriptions(&alice, &[TopicId::from("t2".to_string())].into())
        .await
        .unwrap();

    assert_eq!(
        db.get_subscribers(&TopicId::from("t1".to_string()))
            .await
            .unwrap(),
        vec![bob.clone()]
    );

    let mut subs = db
        .get_subscribers(&TopicId::from("t2".to_string()))
        .await
        .unwrap();
    subs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    assert_eq!(subs, vec![alice, bob]);
}

#[tokio::test]
async fn set_subscriptions_from_empty() {
    let db = create_driver().await;
    let alice = PublicKey::from("alice".to_string());

    db.set_subscriptions(
        &alice,
        &["t1", "t2"]
            .into_iter()
            .map(|s| TopicId::from(s.to_string()))
            .collect(),
    )
    .await
    .unwrap();

    assert_eq!(
        db.get_subscribers(&TopicId::from("t1".to_string()))
            .await
            .unwrap(),
        vec![alice.clone()]
    );
    assert_eq!(
        db.get_subscribers(&TopicId::from("t2".to_string()))
            .await
            .unwrap(),
        vec![alice]
    );
}
