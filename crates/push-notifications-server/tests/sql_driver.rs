use std::collections::HashSet;
use std::sync::Once;
use std::sync::atomic::{AtomicU32, Ordering};

use push_notifications_client::types::{FcmToken, TopicId, VerifyingKey};
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

/// Generate a valid 64-char hex public key from a u8 seed.
fn pub_key(seed: u8) -> VerifyingKey {
    VerifyingKey::from(format!("{:02x}", seed).repeat(32))
}

/// Generate a valid 64-char hex topic ID from a u8 seed.
fn topic(seed: u8) -> TopicId {
    TopicId::from(format!("{:02x}", seed).repeat(32))
}

// --- FCM tokens ---

#[tokio::test]
async fn store_and_get_fcm_token() {
    let db = create_driver().await;
    let alice = pub_key(1);

    db.store_fcm_token(&alice, &FcmToken::from("tok-1".to_string()))
        .await
        .unwrap();

    let tokens = db.get_fcm_tokens(&[alice.clone()]).await.unwrap();
    assert_eq!(
        tokens.get(&alice),
        Some(&FcmToken::from("tok-1".to_string()))
    );
}

#[tokio::test]
async fn get_fcm_token_missing() {
    let db = create_driver().await;
    let nobody = pub_key(99);

    let tokens = db.get_fcm_tokens(&[nobody.clone()]).await.unwrap();
    assert_eq!(tokens.get(&nobody), None);
}

#[tokio::test]
async fn store_fcm_token_overwrites() {
    let db = create_driver().await;
    let alice = pub_key(1);

    db.store_fcm_token(&alice, &FcmToken::from("tok-1".to_string()))
        .await
        .unwrap();
    db.store_fcm_token(&alice, &FcmToken::from("tok-2".to_string()))
        .await
        .unwrap();

    let tokens = db.get_fcm_tokens(&[alice.clone()]).await.unwrap();
    assert_eq!(
        tokens.get(&alice),
        Some(&FcmToken::from("tok-2".to_string()))
    );
}

#[tokio::test]
async fn remove_fcm_token_deletes_token() {
    let db = create_driver().await;
    let alice = pub_key(1);

    db.store_fcm_token(&alice, &FcmToken::from("tok-1".to_string()))
        .await
        .unwrap();

    db.remove_fcm_token(&alice).await.unwrap();

    let tokens = db.get_fcm_tokens(&[alice.clone()]).await.unwrap();
    assert_eq!(tokens.get(&alice), None);
}

#[tokio::test]
async fn remove_fcm_token_nonexistent_is_noop() {
    let db = create_driver().await;

    db.remove_fcm_token(&pub_key(99)).await.unwrap();
}

// --- subscribe / get_subscribers ---

#[tokio::test]
async fn subscribe_and_get_subscribers() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1 = topic(1);
    let t2 = topic(2);

    db.add_topic_subscriptions(&alice, &[t1.clone(), t2.clone()].into_iter().collect())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone()].into())
        .await
        .unwrap();
    assert_eq!(subs.get(&t1).unwrap(), &vec![alice.clone()]);
    assert_eq!(subs.get(&t2).unwrap(), &vec![alice]);
}

#[tokio::test]
async fn subscribe_is_idempotent() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1: HashSet<TopicId> = [topic(1)].into();

    db.add_topic_subscriptions(&alice, &t1).await.unwrap();
    db.add_topic_subscriptions(&alice, &t1).await.unwrap();

    let subs = db.get_subscribers_for_topics(&t1).await.unwrap();
    assert_eq!(subs.get(&topic(1)).unwrap(), &vec![alice]);
}

#[tokio::test]
async fn get_subscribers_empty_topic() {
    let db = create_driver().await;
    let t = topic(99);

    let subs = db
        .get_subscribers_for_topics(&[t.clone()].into())
        .await
        .unwrap();
    assert!(subs.get(&t).unwrap_or(&vec![]).is_empty());
}

#[tokio::test]
async fn multiple_subscribers_same_topic() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let bob = pub_key(2);
    let t1: HashSet<TopicId> = [topic(1)].into();

    db.add_topic_subscriptions(&alice, &t1).await.unwrap();
    db.add_topic_subscriptions(&bob, &t1).await.unwrap();

    let subs = db.get_subscribers_for_topics(&t1).await.unwrap();
    let mut topic_subs = subs.get(&topic(1)).unwrap().clone();
    topic_subs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    assert_eq!(topic_subs, vec![alice, bob]);
}

// --- unsubscribe ---

#[tokio::test]
async fn unsubscribe_removes_subscription() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1 = topic(1);
    let t2 = topic(2);

    db.add_topic_subscriptions(&alice, &[t1.clone(), t2.clone()].into_iter().collect())
        .await
        .unwrap();

    db.remove_topic_subscriptions(&alice, &[t1.clone()].into())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone()].into())
        .await
        .unwrap();
    assert!(subs.get(&t1).unwrap_or(&vec![]).is_empty());
    assert_eq!(subs.get(&t2).unwrap(), &vec![alice]);
}

#[tokio::test]
async fn unsubscribe_only_affects_target_user() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let bob = pub_key(2);
    let t1: HashSet<TopicId> = [topic(1)].into();

    db.add_topic_subscriptions(&alice, &t1).await.unwrap();
    db.add_topic_subscriptions(&bob, &t1).await.unwrap();

    db.remove_topic_subscriptions(&alice, &t1).await.unwrap();

    let subs = db.get_subscribers_for_topics(&t1).await.unwrap();
    assert_eq!(subs.get(&topic(1)).unwrap(), &vec![bob]);
}

// --- set_subscriptions ---

#[tokio::test]
async fn set_subscriptions_replaces_all() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1 = topic(1);
    let t2 = topic(2);
    let t3 = topic(3);
    let t4 = topic(4);

    db.add_topic_subscriptions(
        &alice,
        &[t1.clone(), t2.clone(), t3.clone()].into_iter().collect(),
    )
    .await
    .unwrap();

    db.update_topic_subscriptions(&alice, &[t2.clone(), t4.clone()].into_iter().collect())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone(), t3.clone(), t4.clone()].into())
        .await
        .unwrap();
    assert!(subs.get(&t1).unwrap_or(&vec![]).is_empty());
    assert!(subs.get(&t3).unwrap_or(&vec![]).is_empty());
    assert_eq!(subs.get(&t2).unwrap(), &vec![alice.clone()]);
    assert_eq!(subs.get(&t4).unwrap(), &vec![alice]);
}

#[tokio::test]
async fn set_subscriptions_empty_clears_all() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1 = topic(1);
    let t2 = topic(2);

    db.add_topic_subscriptions(&alice, &[t1.clone(), t2.clone()].into_iter().collect())
        .await
        .unwrap();

    db.update_topic_subscriptions(&alice, &HashSet::new())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone()].into())
        .await
        .unwrap();
    assert!(subs.get(&t1).unwrap_or(&vec![]).is_empty());
    assert!(subs.get(&t2).unwrap_or(&vec![]).is_empty());
}

#[tokio::test]
async fn set_subscriptions_does_not_affect_other_users() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let bob = pub_key(2);
    let t1 = topic(1);
    let t2 = topic(2);
    let t1_t2: HashSet<TopicId> = [t1.clone(), t2.clone()].into();

    db.add_topic_subscriptions(&alice, &t1_t2).await.unwrap();
    db.add_topic_subscriptions(&bob, &t1_t2).await.unwrap();

    db.update_topic_subscriptions(&alice, &[t2.clone()].into())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone()].into())
        .await
        .unwrap();
    assert_eq!(subs.get(&t1).unwrap(), &vec![bob.clone()]);

    let mut t2_subs = subs.get(&t2).unwrap().clone();
    t2_subs.sort_by(|a, b| a.to_string().cmp(&b.to_string()));
    assert_eq!(t2_subs, vec![alice, bob]);
}

#[tokio::test]
async fn set_subscriptions_from_empty() {
    let db = create_driver().await;
    let alice = pub_key(1);
    let t1 = topic(1);
    let t2 = topic(2);

    db.update_topic_subscriptions(&alice, &[t1.clone(), t2.clone()].into_iter().collect())
        .await
        .unwrap();

    let subs = db
        .get_subscribers_for_topics(&[t1.clone(), t2.clone()].into())
        .await
        .unwrap();
    assert_eq!(subs.get(&t1).unwrap(), &vec![alice.clone()]);
    assert_eq!(subs.get(&t2).unwrap(), &vec![alice]);
}
