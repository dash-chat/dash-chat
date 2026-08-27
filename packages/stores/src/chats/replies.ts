import type { DeviceId, Hash } from '../p2panda/types';
import type { MediaAttachment } from '../types';

/** A reply annotation resolved for rendering. The quoted content is frozen at
 * the version that was replied to — later edits of the target never change
 * it — while `scrollTarget` points at the edit-chain root, which is where the
 * (possibly edited) target message is rendered. */
export type MessageReply =
	| {
			kind: 'content';
			author: DeviceId;
			/** Profile name of the quoted author, when their profile is known. */
			authorName?: string;
			text: string;
			media: MediaAttachment | null;
			/** Hash of the rendered message to scroll to (the edit-chain root). */
			scrollTarget?: Hash;
	  }
	/** The target was deleted for everyone (or is unknown locally). The quote
	 * shows only a tombstone; `scrollTarget` is set when the "deleted"
	 * placeholder message is rendered and can be scrolled to. `author` is unset
	 * when the target op itself never reached this peer. */
	| {
			kind: 'deleted';
			author?: DeviceId;
			authorName?: string;
			scrollTarget?: Hash;
	  }
	/** The target was deleted only on this device group — always by us, so the
	 * quote needs no author. It shows the tombstone and
	 * does not scroll. */
	| { kind: 'deleted-for-me' };
