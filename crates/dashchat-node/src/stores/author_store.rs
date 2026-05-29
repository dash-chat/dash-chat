use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use p2panda::VerifyingKey;
use tokio::sync::RwLock;

#[derive(Clone, Debug)]
pub struct AuthorStore<T>(pub(crate) Arc<RwLock<HashMap<T, HashSet<VerifyingKey>>>>);

impl<T: Eq + std::hash::Hash + std::fmt::Debug + Clone> AuthorStore<T> {
    pub fn new() -> Self {
        Self(Arc::new(RwLock::new(HashMap::new())))
    }

    pub async fn add_author(&self, topic: T, verifying_key: impl Into<VerifyingKey>) {
        let mut authors = self.0.write().await;
        let verifying_key = verifying_key.into();
        let pk = VerifyingKey::from(verifying_key);

        authors
            .entry(topic.clone())
            .and_modify(|verifying_keys| {
                if verifying_keys.insert(verifying_key) {
                    tracing::debug!(?topic, ?pk, "added author");
                }
            })
            .or_insert({
                tracing::debug!(?topic, ?pk, "added author (first in topic)");
                let mut verifying_keys = HashSet::new();
                verifying_keys.insert(verifying_key);
                verifying_keys
            });
    }

    pub async fn authors(&self, topic: &T) -> Option<HashSet<VerifyingKey>> {
        let authors = self.0.read().await;
        Some(
            authors
                .get(topic)
                .cloned()?
                .into_iter()
                .map(VerifyingKey::from)
                .collect(),
        )
    }
}

// #[async_trait]
// impl<Topic: Eq + std::hash::Hash + TopicQuery> TopicLogMap<Topic, Topic> for AuthorStore<Topic> {
//     /// During sync other peers are interested in all our append-only logs for a certain topic.
//     /// This method tells the sync protocol which logs we have available from which author for that
//     /// given topic.
//     async fn get(&self, topic: &Topic) -> Option<HashMap<VerifyingKey, Vec<Topic>>> {
//         let authors = self.authors(topic).await;
//         let map = match authors {
//             Some(authors) => {
//                 let mut map = HashMap::with_capacity(authors.len());
//                 for author in authors {
//                     // We write all data of one author into one log for now.
//                     map.insert(author.into(), vec![topic.clone()]);
//                 }
//                 map
//             }
//             None => HashMap::new(),
//         };
//         Some(map)
//     }
// }
