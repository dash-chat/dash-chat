# Encrypting media blobs

p2panda offers encryption of operation payloads (and soon, headers), but we're
on our own for encrypting media blobs since they live entirely outside of
p2panda's purview.

Blob encryption is almost entirely symmetrical with p2panda's message encryption:
AEAD encryption with secret and nonce, with the only difference being that we use
HKDF to get secret for blobs derived from the secret for the containing message.
This harmless step opens the door for sharing the blob secret without revealing anything
else about the message.

The secret (and nonce) used to encode a blob is stored in the `BlobDecryptIndex`,
which are needed since the secret can rotate within the context of a single Topic.

One design choice made for simplicity is that the same blob encrypted in different
topics, or even in the same topic after keys have rotated, will have different
ciphertext and a different hash, so forwarding messages in general will lead to
duplication of data.

The p2panda fork had to add some functions 
(https://github.com/dash-chat/p2panda/blob/c31168fb2a784e1690ee6843650e1bed15e676bb)
to accommodate external data encryption. 
TODO: as a followup, when we have actual message encryption, Space needs a 
`fn with_publish` that accepts a closure taking the secret which is called before the
operation gets published (and encrypted with the same secret), so that secret sharing
atomicity can be enforced with a database transaction.

