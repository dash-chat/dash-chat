export type Signature = Uint8Array;
export type PublicKey = string;
export type Hash = string;
export type LogId = string;
export type TopicId = string;

export interface Operation<E = void> {
	hash: Hash;
	header: Header<E>;
	body: Body | undefined;
}

export interface Header<E = void> {
	/// Operation format version, allowing backwards compatibility when specification changes.
	version: number;

	/// Author of this operation.
	public_key: PublicKey;

	/// Signature by author over all fields in header, providing authenticity.
	signature: Signature | undefined;

	/// Number of bytes of the body of this operation, must be zero if no body is given.
	payload_size: number;

	/// Hash of the body of this operation, must be included if payload_size is non-zero and
	/// omitted otherwise.
	///
	/// Keeping the hash here allows us to delete the payload (off-chain data) while retaining the
	/// ability to check the signature of the header.
	payload_hash: Hash | undefined;

	/// Time in microseconds since the Unix epoch.
	timestamp: number;

	/// Number of operations this author has published to this log, begins with 0 and is always
	/// incremented by 1 with each new operation by the same author.
	seq_num: number;

	/// Hash of the previous operation of the same author and log. Can be omitted if first
	/// operation in log.
	backlink: Hash | undefined;

	/// List of hashes of the operations we refer to as the "previous" ones. These are operations
	/// from other authors. Can be left empty if no partial ordering is required or no other
	/// author has been observed yet.
	previous: Array<Hash>;

	/// Custom meta data.
	// extensions: E>, | undefined
}

export type Body = Uint8Array;

export interface Lifetime {
	not_before: number;
	not_after: number;
}

export type DeviceId = PublicKey;
export type AgentId = PublicKey;

export type PreKey = [PublicKey, Lifetime];

export type XSignature = string; // TODO: check this is correct

export interface LongTermKeyBundle {
	identity_key: PublicKey;
	signed_prekey: PreKey;
	prekey_signature: XSignature;
}
