import { fromByteArray, toByteArray } from 'base64-js';
// @ts-ignore
import { decode, encode } from 'cbor-web';

import { ContactCode } from '../types';

export function encodeContactCode(contactCode: ContactCode, versionHint?: string): string {
	const bin = encode([
		contactCode.device_pubkey,
		contactCode.agent_id,
		contactCode.inbox_topic,
		contactCode.share_intent,
	]);
	const base64 = fromByteArray(bin);
	if (versionHint) {
		// Append version hint after the base64 padding; stripped during decode
		const withoutPadding = base64.replace(/=+$/, '');
		return `${withoutPadding}=${versionHint}`;
	}
	return base64;
}

export function decodeContactCode(contactCodeString: string): ContactCode {
	// Strip anything after (and including) the first '=' to ignore version hints
	const base64 = contactCodeString.split('=')[0];
	const bin = toByteArray(base64);
	const [device_pubkey, agent_id, inbox_topic, share_intent] = decode(bin);
	return {
		device_pubkey,
		agent_id,
		inbox_topic,
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
