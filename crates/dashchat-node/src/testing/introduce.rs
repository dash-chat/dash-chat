use std::time::Duration;

use futures::future::join_all;

use crate::{
    testing::{test_node::TestNode, wait_for},
    topic::TopicId,
};

pub async fn introduce_and_wait(nodes: impl IntoIterator<Item = &TestNode>) {
    #[cfg(feature = "p2p")]
    unimplemented!("re-implement when p2p sync is available");

    // let networks = networks.into_iter().collect::<Vec<_>>();
    // let expected_peers = networks.len() - 1;
    // introduce(networks.clone()).await;
    // wait_for(
    //     Duration::from_millis(100),
    //     Duration::from_secs(10),
    //     || async {
    //         let peers = join_all(
    //             networks
    //                 .iter()
    //                 .map(|n| async { n.known_peers().await.unwrap().len() }),
    //         )
    //         .await;
    //         match peers.iter().all(|p| *p == expected_peers) {
    //             true => Ok(()),
    //             false => Err(peers),
    //         }
    //     },
    // )
    // .await
    // .unwrap();
}

pub async fn introduce(nodes: impl IntoIterator<Item = &TestNode>) {
    #[cfg(feature = "p2p")]
    unimplemented!("re-implement when p2p sync is available");

    // let networks = networks.into_iter().collect::<Vec<_>>();
    // for m in networks.iter() {
    //     for n in networks.iter() {
    //         if m.node_id() == n.node_id() {
    //             continue;
    //         }
    //         let m_addr = m.endpoint().node_addr().await.unwrap();
    //         let n_addr = n.endpoint().node_addr().await.unwrap();

    //         m.add_peer(NodeAddress {
    //             public_key: p2panda_core::PublicKey::from_bytes(n_addr.node_id.as_bytes())
    //                 .expect("already validated public key"),
    //             direct_addresses: n_addr
    //                 .direct_addresses
    //                 .iter()
    //                 .map(|addr| addr.to_owned())
    //                 .collect(),
    //             relay_url: None, // n_addr.relay_url.map(to_relay_url),
    //         })
    //         .await
    //         .unwrap();

    //         n.add_peer(NodeAddress {
    //             public_key: p2panda_core::PublicKey::from_bytes(m_addr.node_id.as_bytes())
    //                 .expect("already validated public key"),
    //             direct_addresses: m_addr
    //                 .direct_addresses
    //                 .iter()
    //                 .map(|addr| addr.to_owned())
    //                 .collect(),
    //             relay_url: None, // n_addr.relay_url.map(to_relay_url),
    //         })
    //         .await
    //         .unwrap();
    //     }
    // }
}
