#[cfg(test)]
mod tests {
    use dashchat_compat::{VersionConvert, VersionConvertError};

    use mailbox_client::mem::MemMailbox;
    use maplit::btreeset;
    use p2panda_core::cbor::{decode_cbor, encode_cbor};

    use crate::{
        ShareIntent,
        chat::*,
        compat::Capabilities,
        testing::{PollConfig, TestNode, TestNodeConfig},
    };

    #[test]
    fn chat_message_v0_roundtrip() {
        let v0 = ChatMessageContent::unversioned("hello");
        let bytes = encode_cbor(&v0).unwrap();
        let bare_bytes = encode_cbor(&ChatMessageContentV0::from("hello".to_string())).unwrap();
        assert_eq!(bytes, bare_bytes);
        let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, v0);
    }

    #[test]
    fn chat_message_v1_roundtrip() {
        let v1 = ChatMessageContent::text_only("hello");
        let bytes = encode_cbor(&v1).unwrap();
        let decoded: ChatMessageContent = decode_cbor(bytes.as_slice()).unwrap();
        assert_eq!(decoded, v1);
    }

    #[test]
    fn chat_message_getters() {
        let v0 = ChatMessageContent::unversioned("hello");
        assert_eq!(v0.message(), "hello");
        assert!(v0.media_meta().is_none());
        let v1 = ChatMessageContent::text_only("world");
        assert_eq!(v1.message(), "world");
        assert!(v1.media_meta().is_none());
    }

    #[test]
    fn version_convert_v1_to_v0() {
        let v1 = ChatMessageContent::text_only("hello");
        let v0 = v1.to_version(&Capabilities::zero()).unwrap();
        assert_eq!(v0, ChatMessageContent::unversioned("hello"));
    }

    #[test]
    fn version_convert_v0_to_v1() {
        let v0 = ChatMessageContent::unversioned("hello");
        let c = Capabilities { messaging: 1 };
        let v1 = v0.to_version(&c).unwrap();
        assert_eq!(v1.message(), "hello");
        assert!(v1.media_meta().is_none());
    }

    #[test]
    fn version_convert_v1_to_v0_lossy() {
        let v1_empty = ChatMessageContent::new("anything", Some(MediaMetaCollection::from(vec![])));
        let result = v1_empty.to_version(&Capabilities::zero());
        assert_eq!(result, Err(VersionConvertError::Lossy));
    }

    #[test]
    fn version_convert_unknown_version() {
        let v0 = ChatMessageContent::unversioned("hello");
        let result = v0.to_version(&Capabilities { messaging: 99 });
        assert_eq!(result, Err(VersionConvertError::UnknownVersion));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn get_set_capabilities() {
        dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

        let mut alice_config = TestNodeConfig::default();
        let bobbi_config = TestNodeConfig::default();
        alice_config.node_config.capabilities = Capabilities::zero();

        let mailbox = MemMailbox::new();
        let alice = TestNode::new(alice_config, "alice")
            .await
            .add_mailbox_client(mailbox.client())
            .await;
        let bobbi = TestNode::new(bobbi_config, "bobbi")
            .await
            .add_mailbox_client(mailbox.client())
            .await;

        let ac = alice
            .local_store
            .get_capabilities(alice.device_id())
            .await
            .unwrap()
            .unwrap();
        let bc = bobbi
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(ac, Capabilities::zero());
        assert_eq!(bc, Capabilities::current());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn messaging_v0_to_v1() {
        dashchat_node::testing::setup_tracing(&["dashchat=warn"], true);

        let mut alice_config = TestNodeConfig::default();
        let bobbi_config = TestNodeConfig::default();
        alice_config.node_config.capabilities = Capabilities::zero();

        let poll = PollConfig::default();
        let mailbox = MemMailbox::new();
        let alice = TestNode::new(alice_config, "alice")
            .await
            .add_mailbox_client(mailbox.client())
            .await;
        let bobbi = TestNode::new(bobbi_config, "bobbi")
            .await
            .add_mailbox_client(mailbox.client())
            .await;

        println!("alice: {:?}", alice.device_id().to_hex());
        println!("bobbi: {:?}", bobbi.device_id().to_hex());

        alice
            .behavior()
            .initiate_and_establish_contact(&bobbi, ShareIntent::AddContact)
            .await
            .unwrap();

        let topic = alice.direct_chat_topic(bobbi.agent_id());

        poll.consistency([&alice, &bobbi], &[topic.into()])
            .await
            .unwrap();

        // Both nodes see each others' capabilities
        let alice_bobbi_caps = alice
            .local_store
            .get_capabilities(bobbi.device_id())
            .await
            .unwrap()
            .unwrap();
        let bobbi_alice_caps = bobbi
            .local_store
            .get_capabilities(alice.device_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(alice_bobbi_caps, Capabilities::current());
        assert_eq!(bobbi_alice_caps, Capabilities::zero());

        poll.consistency([&alice, &bobbi], &[topic.into()])
            .await
            .unwrap();

        let alice_members = alice.get_group_members(topic).await.unwrap();
        let bobbi_members = bobbi.get_group_members(topic).await.unwrap();
        let expected_members = btreeset![
            (alice.device_id(), p2panda_auth::Access::write()),
            (bobbi.device_id(), p2panda_auth::Access::write())
        ];

        assert_eq!(alice_members, expected_members);
        assert_eq!(bobbi_members, expected_members);

        // Both nodes return zero capabilities because alice is the limiting factor.
        let alice_caps = alice
            .get_group_capabilities(topic)
            .await
            .unwrap()
            .0
            .unwrap();
        let bobbi_caps = bobbi
            .get_group_capabilities(topic)
            .await
            .unwrap()
            .0
            .unwrap();
        assert_eq!(alice_caps, Capabilities::zero());
        assert_eq!(bobbi_caps, Capabilities::zero());

        let chat = alice.direct_chat_topic(bobbi.agent_id());
        alice
            .send_message_raw(chat, ChatMessageContent::unversioned("Hello"))
            .await
            .unwrap();
        bobbi
            .send_message_raw(chat, ChatMessageContent::text_only("Hello back"))
            .await
            .unwrap();

        poll.wait_for(|| async {
            let ma = alice.get_messages(chat).await.unwrap();
            let mb = bobbi.get_messages(chat).await.unwrap();
            if ma.len() == 2 && mb.len() == 2 {
                Ok(())
            } else {
                Err("messages not received")
            }
        })
        .await
        .unwrap();

        // All messages should be at version 0 because of alice's zero capability.
        let messages_alice = alice.get_messages(chat).await.unwrap();
        let messages_bobbi = bobbi.get_messages(chat).await.unwrap();
        assert_eq!(messages_alice, messages_bobbi);
        assert_eq!(
            messages_alice
                .into_iter()
                .map(|m| m.content)
                .collect::<Vec<_>>(),
            vec![
                ChatMessageContent::unversioned("Hello"),
                ChatMessageContent::unversioned("Hello back"),
            ]
        );
    }
}
