use dashchat_node::{DeviceId, Payload, TopicId};
use p2panda::operation::{Header, LogId};
use p2panda::{Hash, VerifyingKey};
use p2panda_auth::processor::GroupsArgs;
use p2panda_core::cbor::decode_cbor;
use p2panda_core::Timestamp;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use tauri::State;

use crate::app_node::AppNode;

/// Serialize a `Timestamp` (microseconds) as milliseconds since the UNIX epoch
/// so JS can pass it straight to `new Date(ms)`.
fn serialize_timestamp_as_millis<S: Serializer>(ts: &Timestamp, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_u64(u64::from(*ts) / 1_000)
}

fn deserialize_timestamp_from_millis<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<Timestamp, D::Error> {
    let millis = u64::deserialize(d)?;
    Ok(Timestamp::new(millis * 1_000))
}

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct SimplifiedOperation {
    pub hash: Hash,
    pub header: SimplifiedHeader,
    pub body: Option<serde_json::Value>,
}

// #[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
// pub struct SimplifiedSpacesOperation {
//     // hash: Hash,
//     pub header: SimplifiedHeader,
//     pub events: Vec<Event<ChatId, TestConditions>>,
// }

#[derive(Debug, Serialize, Deserialize, Eq, PartialEq, Clone)]
pub struct SimplifiedHeader {
    /// Author of this operation.
    verifying_key: VerifyingKey,

    /// Milliseconds since the UNIX epoch when the operation was created.
    #[serde(
        serialize_with = "serialize_timestamp_as_millis",
        deserialize_with = "deserialize_timestamp_from_millis"
    )]
    timestamp: Timestamp,

    /// Number of operations this author has published to this log, begins with 0 and is always
    /// incremented by 1 with each new operation by the same author.
    seq_num: u64,

    /// Hash of the previous operation of the same author and log. Can be omitted if first
    /// operation in log.
    backlink: Option<Hash>,

    /// List of hashes of the operations we refer to as the "previous" ones. These are operations
    /// from other authors. Can be left empty if no partial ordering is required or no other
    /// author has been observed yet.
    previous: Vec<Hash>,

    topic_id: TopicId,

    /// p2panda-auth group-control extension, when this operation is a group action
    /// (Create / Add / Remove / Promote / Demote) rather than a chat payload.
    #[serde(skip_serializing_if = "Option::is_none")]
    auth: Option<GroupsArgs>,
}

impl SimplifiedHeader {
    /// Convert a p2panda::Header into a SimplifiedHeader.
    ///
    /// As a p2panda::Header does not contain the raw topic (only the hashed representation in the
    /// form of a LogId) we need to pass this in as a separate argument.
    pub fn from_header(topic: TopicId, header: Header) -> Self {
        // Only operations contain groups args in their extension have dependency requirements.
        let previous = header
            .extensions
            .groups_args
            .as_ref()
            .map(|args| args.dependencies.clone())
            .unwrap_or_default();
        SimplifiedHeader {
            verifying_key: header.verifying_key,
            timestamp: header.timestamp,
            seq_num: header.seq_num,
            backlink: header.backlink,
            previous,
            topic_id: topic,
            auth: header.extensions.groups_args.clone(),
        }
    }
}

// pub fn decode_spaces_args(spaces_args: SpacesArgs) -> Result<Option<serde_json::Value>, String> {
//     match spaces_args {
//         p2panda_spaces::SpacesArgs::Application {
//             space_id,
//             space_dependencies,
//             group_secret_id,
//             nonce,
//             ciphertext,
//         } => {
//             todo!()
//         }
//         // p2panda_spaces::SpacesArgs::Auth { control_message, auth_dependencies } => {

//         // },
//         _ => todo!(),
//     }
// }

// pub fn decode_body(body: Body) -> Result<serde_json::Value, String> {
//     let _bytes = body.to_bytes();
//     // let Ok(Payload::Space(args)) = decode_cbor(&bytes[..]) else {
//     //     return Ok(decode_cbor(&bytes[..]).map_err(|err| format!("{err:?}"))?);
//     // };

//     let values: Vec<serde_json::Value> = vec![];

//     // if let Some(value) = decode_spaces_args(args)? {
//     //     values.push(value);
//     // }

//     Ok(serde_json::Value::Array(values))
// }

pub fn simplify(
    topic: TopicId,
    hash: Hash,
    header: Header,
    body: Option<p2panda_core::Body>,
) -> anyhow::Result<SimplifiedOperation> {
    let body: Option<serde_json::Value> = match body {
        Some(b) => {
            let payload: Payload = decode_cbor(&b.to_bytes()[..])?;

            // if let Payload::Chat(dashchat_node::ChatPayload::Space(spaces_messages)) = payload {
            //     let mut all_events: Vec<SimplifiedEvent> = vec![];

            //     for message in spaces_messages {
            //         // let events = node.manager.process(&message).await?;
            //         let map = node.nodestate.spaces_events.read().await;
            //         let Some(events) = map.get(&message.hash) else {
            //             continue;
            //         };
            //         let mut simplified_events = events
            //             .into_iter()
            //             .map(simplify_event)
            //             .collect::<anyhow::Result<Vec<Option<SimplifiedEvent>>>>()?
            //             .into_iter()
            //             .filter_map(|e| e)
            //             .collect();

            //         all_events.append(&mut simplified_events);
            //     }

            //     Some(serde_json::to_value(all_events)?)
            // } else {
            Some(serde_json::to_value(payload)?)
            // }
        }
        _ => None,
    };

    let operation = SimplifiedOperation {
        hash,
        header: SimplifiedHeader::from_header(topic, header),
        body,
    };

    Ok(operation)
}

#[tauri::command]
pub async fn get_log(
    topic_id: TopicId,
    author: DeviceId,
    app_node: State<'_, AppNode>,
) -> Result<Vec<SimplifiedOperation>, String> {
    let node = app_node.get().await?;
    let log = node
        .op_store
        .get_log(&author, &LogId::from_topic(topic_id), None)
        .await
        .map_err(|e| format!("Failed to get log: {e:?}"))?;

    let simplified_log = log
        .into_iter()
        .map(|op| simplify(topic_id, op.hash, op.header, op.body))
        .collect::<anyhow::Result<Vec<SimplifiedOperation>>>()
        .map_err(|err| format!("{err:?}"))?;

    Ok(simplified_log)
}

#[tauri::command]
pub async fn get_authors(
    topic_id: TopicId,
    app_node: State<'_, AppNode>,
) -> Result<std::collections::HashSet<DeviceId>, String> {
    let node = app_node.get().await?;
    let authors = node
        .op_store
        .get_authors(LogId::from_topic(topic_id))
        .await
        .map_err(|e| format!("Failed to get log: {e:?}"))?;
    Ok(authors)
}
