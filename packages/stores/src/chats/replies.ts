import type { DeviceId, Hash } from '../p2panda/types';
import type { MediaAttachment, Payload } from '../types';

/** A reply annotation resolved for rendering. The quoted content is frozen at
 * the version that was replied to — later edits of the target never change
 * it — while `scrollTarget` points at the edit-chain root, which is where the
 * (possibly edited) target message is rendered. */
export type MessageReply =
	| {
			kind: 'content';
			author: DeviceId;
			text: string;
			media: MediaAttachment | null;
			/** Hash of the rendered message to scroll to (the edit-chain root). */
			scrollTarget?: Hash;
	  }
	/** The target was deleted for everyone (or is unknown locally). The quote
	 * shows only a tombstone; `scrollTarget` is set when the "deleted"
	 * placeholder message is rendered and can be scrolled to. `author` is unset
	 * when the target op itself never reached this peer. */
	| { kind: 'deleted'; author?: DeviceId; scrollTarget?: Hash }
	/** The target was deleted only on this device group — always by us, so the
	 * quote needs no author. It shows the tombstone and
	 * does not scroll. */
	| { kind: 'deleted-for-me' };

/** The author of the message a reply quotes, when it is known: a quote of a
 * message this peer never received has none, and a delete-for-me quote names
 * no one but us. */
export function replyAuthor(
	reply: MessageReply | undefined,
): DeviceId | undefined {
	if (reply === undefined || reply.kind === 'deleted-for-me') return undefined;
	return reply.author;
}
