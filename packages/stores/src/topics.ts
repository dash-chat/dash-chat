import { blake3Hash } from '@webbuf/blake3';
import { WebBuf } from 'webbuf';

import { AgentId, PublicKey, TopicId } from './p2panda/types';

const fromHexString = (hexString: string) =>
	Uint8Array.from(hexString.match(/.{1,2}/g)!.map(byte => parseInt(byte, 16)));

// export function personalTopicFor(actorId: AgentId): TopicId {
// 	const hash = blake3Hash(WebBuf.fromHex(actorId));
// 	const a = new Uint8Array(32).fill(255);

// 	const hex = WebBuf.fromUint8Array(a).toHex();

// 	return hex;
// }

export function personalTopicFor(actorId: AgentId): TopicId {
	return actorId;
}
// export function personalTopicFor(actorId: ActorId): TopicId {
 	// const bytes = fromHexString(actorId);
 //	const hash = blake3Hash(WebBuf.fromHex(actorId));
 	// console.log(WebBuf.fromHex(actorId), hash.buf);
 //	return `${hash.buf.toHex()}`;
// }
// export function personalTopicFor(chatActorId: ActorId): TopicId {
// 	return chatActorId;
// }
