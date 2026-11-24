# mailbox

## sync

- for each (topic, author) in op store:
    - check highest seq num stored vs what mailbox has, via check() API call
        - specifically, this is the highest contiguous seq num stored on the mailbox, so that gaps can be filled in
    - send encrypted ops to mailbox keyed by (topic, author, seq)

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

- for each topic:
    - ask for all ops filtered by `[(author, seq)]` (use `LogStore::get_log_heights`)
    - decrypt using p2panda-groups encryption state
    - ingest whatever comes back and was decryptable