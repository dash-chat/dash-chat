use dashchat_node::{topic::TopicId, DeviceId, Header, Node, Payload, Topic};
use p2panda_core::{cbor::decode_cbor, Body, Hash, PublicKey};
use serde::{Deserialize, Serialize};
use tauri::State;

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
    public_key: PublicKey,

    /// Time in microseconds since the Unix epoch.
    timestamp: u64,

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

    topic_id: Topic,
}

impl From<Header> for SimplifiedHeader {
    fn from(header: Header) -> SimplifiedHeader {
        SimplifiedHeader {
            public_key: header.public_key,
            timestamp: header.timestamp,
            seq_num: header.seq_num,
            backlink: header.backlink,
            previous: header.previous,
            topic_id: Topic::untyped(*header.extensions.topic),
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
    hash: Hash,
    header: Header,
    body: Option<Body>,
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
        header: SimplifiedHeader::from(header),
        body,
    };

    Ok(operation)
}

#[tauri::command]
pub async fn get_log(
    topic_id: Topic,
    author: DeviceId,
    node: State<'_, Node>,
) -> Result<Vec<SimplifiedOperation>, String> {
    let log = node
        .get_log(TopicId::from(topic_id), author)
        .await
        .map_err(|e| format!("Failed to get log: {e:?}"))?;

    let simplified_log = log
        .into_iter()
        .map(|(header, body)| simplify(header.hash(), header, body))
        .collect::<anyhow::Result<Vec<SimplifiedOperation>>>()
        .map_err(|err| format!("{err:?}"))?;

    Ok(simplified_log)
}

#[tauri::command]
pub async fn get_authors(
    topic_id: Topic,
    node: State<'_, Node>,
) -> Result<std::collections::HashSet<DeviceId>, String> {
    let authors = node
        .get_authors(TopicId::from(topic_id))
        .await
        .map_err(|e| format!("Failed to get log: {e:?}"))?;
    Ok(authors)
}
