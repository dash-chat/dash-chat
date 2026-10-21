# mailbox

## sync

- for each topic, resolve its logs the way p2panda does (`TopicStore::resolve`): a set of (author, log id) pairs
    - a log can belong to several topics (a member's key bundle log belongs to every space they are in)
- for each (author, log id):
    - check highest seq num stored vs what mailbox has, via check() API call
        - specifically, this is the highest contiguous seq num stored on the mailbox, so that gaps can be filled in
    - send encrypted ops to mailbox keyed by (author, log id, seq), once however many topics the log belongs to
- tell the mailbox which logs belong to which topic, so it can resolve the topic for the next node

This is important for all nodes to do, even for ops they didn't author, because every node should be responsible for keeping all mailboxes in sync. 

We want:
- alice bob and carol are all in a group chat
- alice and bob are both on mailbox X
- bob and carol are both on mailbox Y
- alice syncs to mailbox X and goes offline
- bob fetches alice's ops
- bob syncs with mailbox Y
- carol gets alice's ops

## poll

- ask for a set of topics, filtered by the heights `[(author, log id, seq)]` of the logs held locally
    - the mailbox resolves each topic to the logs it knows under it and sends everything above the heights
    - a log not in the heights comes back in full: this is how a joiner discovers the control logs
      (space, group, key bundles) it cannot name before processing them
- route each op to every requested topic the mailbox says its log belongs to
- decrypt using p2panda-groups encryption state
- ingest whatever comes back and was decryptable