import { AgentId, DeviceId } from '../p2panda/types';

const PENDING_PREFIX = 'pending-device:';

/**
 * The id used to address a direct chat in routes and summaries. Normally an
 * established contact's `AgentId`, but for an outgoing contact request whose
 * ack hasn't arrived yet we only know the owner's `DeviceId`, so the key is
 * tagged `pending-device:<device_pubkey>` to make it unmistakable that it is a
 * device pubkey, not an agent id.
 */
export type ChatKey = string;

export function pendingChatKey(devicePubkey: DeviceId): ChatKey {
	return `${PENDING_PREFIX}${devicePubkey}`;
}

export function isPendingChatKey(key: ChatKey): boolean {
	return key.startsWith(PENDING_PREFIX);
}

/**
 * Extract the device pubkey from a pending chat key, or `undefined` if the key
 * is a regular agent-keyed chat.
 */
export function pendingChatKeyDevice(key: ChatKey): DeviceId | undefined {
	return isPendingChatKey(key) ? key.slice(PENDING_PREFIX.length) : undefined;
}

/** Narrow a chat key to an `AgentId`, or `undefined` for a pending key. */
export function chatKeyAgentId(key: ChatKey): AgentId | undefined {
	return isPendingChatKey(key) ? undefined : key;
}
