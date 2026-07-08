import { fromByteArray, toByteArray } from 'base64-js';
// @ts-ignore
import { decode, encode } from 'cbor-web';

import { ContactCode } from '../types';

function hexToBytes(hex: string): Uint8Array {
	return Uint8Array.from(hex.match(/.{1,2}/g)!.map(byte => parseInt(byte, 16)));
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
}

// The hex string keys (device_pubkey, inbox_topic.topic) are converted to raw
// bytes before CBOR encoding so each 32-byte key costs 32 bytes rather than a
// 64-char text string, roughly halving the encoded contact code.
export function encodeContactCode(contactCode: ContactCode): string {
	const inboxTopic = contactCode.inbox_topic
		? [
				contactCode.inbox_topic.expires_at,
				hexToBytes(contactCode.inbox_topic.topic),
			]
		: null;
	const bin = encode([
		hexToBytes(contactCode.device_pubkey),
		inboxTopic,
		contactCode.share_intent,
	]);
	return fromByteArray(bin);
}

export function decodeContactCode(contactCodeString: string): ContactCode {
	const bin = toByteArray(contactCodeString);
	const [device_pubkey, inbox_topic, share_intent] = decode(bin);
	return {
		device_pubkey: bytesToHex(device_pubkey),
		inbox_topic: inbox_topic
			? { expires_at: inbox_topic[0], topic: bytesToHex(inbox_topic[1]) }
			: undefined,
		share_intent,
	};
}

export const compress = async (
	str: string,
	encoding = 'gzip' as CompressionFormat,
): Promise<ArrayBuffer> => {
	const byteArray = new TextEncoder().encode(str);
	const cs = new CompressionStream(encoding);
	const writer = cs.writable.getWriter();
	writer.write(byteArray);
	writer.close();
	return new Response(cs.readable).arrayBuffer();
};

export const decompress = async (
	byteArray: ArrayBuffer,
	encoding = 'gzip' as CompressionFormat,
): Promise<string> => {
	const cs = new DecompressionStream(encoding);
	const writer = cs.writable.getWriter();
	writer.write(byteArray);
	writer.close();
	const arrayBuffer = await new Response(cs.readable).arrayBuffer();
	return new TextDecoder().decode(arrayBuffer);
};
