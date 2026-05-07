import type { Hash, PublicKey } from './types';

export interface SimplifiedOperation<B> {
	hash: Hash;
	header: SimplifiedHeader;
	body: B | undefined;
}

export interface SimplifiedHeader {
	/// Author of this operation.
	public_key: PublicKey;

	/// Milliseconds since the Unix epoch (suitable for `new Date(ms)`).
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

	topic_id: Hash;

	/// Custom meta data.
	// extensions: E>, | undefined
}
