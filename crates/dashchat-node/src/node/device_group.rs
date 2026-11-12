use super::*; 

#[cfg(test)]
mod tests {
    use crate::testing::*;
    use crate::*;
    use p2panda::operation::LogId;

    fn forged_profile() -> Profile {
        Profile {
            name: "Forged".to_string(),
            surname: None,
            avatar: None,
            about: None,
        }
    }

    /// A device-group payload published into a chat space (whose members are
    /// not my devices) must not touch my state, even when its author is a
    /// member of that chat.
    #[tokio::test(flavor = "multi_thread")]
    async fn device_group_ops_on_other_topics_are_ignored() {
        let alice = TestNode::new(NodeConfig::testing().no_p2p().no_blob_sync(), "alice").await;
        let chat_id = alice
            .create_chat_group(std::collections::BTreeMap::new())
            .await
            .unwrap();

        alice
            .publish(
                chat_id,
                Payload::DeviceGroup(DeviceGroupPayload::SetProfile(forged_profile())),
                None,
            )
            .await
            .unwrap();

        assert_eq!(alice.my_profile().await.unwrap().unwrap().name, "alice");
    }
}
