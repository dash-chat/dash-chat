//! What p2panda does when someone who knows a space id publishes their own
//! Create for it: does the second Create root merge, and does its author gain
//! membership or admin rights in the eyes of the original members?

use dashchat_node::membership::membership_action;
use dashchat_node::{testing::*, *};
use p2panda_auth::Access;
use p2panda_auth::group::GroupMember;

const TRACING_FILTER: [&str; 5] = [
    "space_second_create=info",
    "dashchat=info",
    "p2panda_stream=warn",
    "p2panda_auth=warn",
    "p2panda_spaces=warn",
];

fn names(
    members: &std::collections::BTreeSet<(DeviceId, Access)>,
    nodes: &[&TestNode],
) -> Vec<String> {
    members
        .iter()
        .map(|(id, access)| {
            let name = nodes
                .iter()
                .find(|n| n.device_id() == *id)
                .map(|n| format!("{:?}", n.device_id().aliased()))
                .unwrap_or_else(|| "unknown".into());
            format!("{name}:{:?}", access.level)
        })
        .collect()
}

async fn log_state(label: &str, chat_id: ChatId, nodes: &[&TestNode]) {
    for node in nodes {
        let members = node.get_group_members(chat_id).await.unwrap();
        let ops = node.get_group_control_ops(chat_id).await.unwrap();
        let actions: Vec<String> = ops
            .iter()
            .map(|op| {
                format!(
                    "{:?}#{}:{}",
                    DeviceId::from(op.header.verifying_key).aliased(),
                    op.header.seq_num,
                    membership_action(&op.header)
                        .map(|a| format!("{a:?}").split(' ').next().unwrap().to_string())
                        .unwrap_or_default()
                )
            })
            .collect();
        let messages: Vec<String> = node
            .get_messages(chat_id)
            .await
            .unwrap()
            .iter()
            .map(|m| m.content.message().to_string())
            .collect();
        tracing::info!(
            node = ?node.device_id().aliased(),
            members = ?names(&members, nodes),
            control_ops = ?actions,
            ?messages,
            "{label}"
        );
    }
}

/// Alice makes a group with Bobbi. Cammy, never a member, learns the chat id and
/// publishes her own Create for it naming herself manager and the others writers,
/// then the three sync.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "documents a p2panda-auth hole: the second Create merges and its author takes over the group"]
async fn test_outsider_publishes_second_create_for_existing_group() {
    dashchat_node::testing::setup_tracing(&TRACING_FILTER, true);

    let config = NodeConfig::testing().random_network_id();
    let alice = TestNode::new(config.clone(), "alice").await;
    let bobbi = TestNode::new(config.clone(), "bobbi").await;
    let cammy = TestNode::new(config.clone(), "cammy").await;

    introduce_peers([&alice, &bobbi]).await.unwrap();
    alice
        .behavior()
        .initiate_and_establish_contact(&bobbi)
        .await
        .unwrap();

    let chat_id = alice
        .create_chat_group(maplit::btreemap! {
            *bobbi.device_id() => Access::write(),
        })
        .await
        .unwrap()
        .alias_named("groupchat");
    alice
        .send_message_raw(chat_id, "before cammy".into())
        .await
        .unwrap();
    bobbi
        .behavior()
        .accept_next_group_invitation()
        .await
        .unwrap();
    PollConfig::default()
        .consistency([&alice, &bobbi], &[chat_id.into()])
        .await
        .unwrap();

    cammy
        .save_keybundle(alice.keybundle().await.unwrap())
        .await
        .unwrap();
    cammy
        .save_keybundle(bobbi.keybundle().await.unwrap())
        .await
        .unwrap();
    cammy
        .create_space_at(
            chat_id,
            vec![
                (
                    GroupMember::Individual(*cammy.device_id()),
                    Access::manage(),
                ),
                (GroupMember::Individual(*alice.device_id()), Access::write()),
                (GroupMember::Individual(*bobbi.device_id()), Access::write()),
            ],
        )
        .await
        .unwrap();
    cammy
        .send_message_raw(chat_id, "from cammy".into())
        .await
        .unwrap();
    log_state("before sync", chat_id, &[&alice, &bobbi, &cammy]).await;

    introduce_peers([&alice, &bobbi, &cammy]).await.unwrap();

    let synced = PollConfig::seconds(20)
        .wait_for(|| async {
            let n = alice.get_group_control_ops(chat_id).await.unwrap().len();
            if n >= 5 {
                Ok(())
            } else {
                Err(format!("alice has {n} control ops"))
            }
        })
        .await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    log_state("after sync", chat_id, &[&alice, &bobbi, &cammy]).await;
    tracing::info!(?synced, "did cammy's control ops reach alice");

    alice
        .send_message_raw(chat_id, "after cammy".into())
        .await
        .unwrap();
    let cammy_read = PollConfig::seconds(10)
        .wait_for(|| async {
            let msgs = cammy.get_messages(chat_id).await.unwrap();
            if msgs.iter().any(|m| m.content.message() == "after cammy") {
                Ok(())
            } else {
                Err(msgs.len())
            }
        })
        .await;
    let removal = cammy.remove_group_member(chat_id, *alice.device_id()).await;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    log_state(
        "after cammy's removal attempt",
        chat_id,
        &[&alice, &bobbi, &cammy],
    )
    .await;
    tracing::info!(?cammy_read, removal = ?removal.is_ok(), "cammy's reach");

    let alice_members = alice.get_group_members(chat_id).await.unwrap();
    let cammy_is_member = alice_members.iter().any(|(id, _)| *id == cammy.device_id());
    let alice_messages = alice.get_messages(chat_id).await.unwrap();
    let alice_sees_cammy = alice_messages
        .iter()
        .any(|m| m.content.message() == "from cammy");

    assert!(
        !cammy_is_member,
        "alice's group now counts cammy as a member: {alice_members:?}"
    );
    assert!(
        !alice_sees_cammy,
        "alice shows cammy's message: {alice_messages:?}"
    );
    assert!(
        cammy_read.is_err(),
        "cammy decrypted alice's message sent after the merge"
    );
    assert!(
        alice_members.iter().any(|(id, _)| *id == alice.device_id()),
        "cammy removed alice from her own group"
    );
}
