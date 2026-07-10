import { fromByteArray, toByteArray } from 'base64-js';
// @ts-ignore
import { decode, encode } from 'cbor-web';

import { ContactCode } from '../types';

function hexToBytes(hex: string): Uint8Array {
	return Uint8Array.from(
		(hex.match(/.{1,2}/g) ?? []).map(byte => parseInt(byte, 16)),
	);
}

function toBytes(value: string | Uint8Array | number[]): Uint8Array {
	if (typeof value === 'string') {
		return hexToBytes(value);
	}
	if (value instanceof Uint8Array) {
		return value;
	}
	return Uint8Array.from(value);
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
}

// The hex string keys are converted to raw bytes before CBOR encoding.
// The inbox topic is replaced by an 8-byte nonce, cutting the code length by ~30%.
export function encodeContactCode(contactCode: ContactCode): string {
	const bin = encode([
		toBytes(contactCode.device_pubkey),
		contactCode.inbox_nonce ? toBytes(contactCode.inbox_nonce) : null,
		contactCode.share_intent,
	]);
	return fromByteArray(bin);
}

export function decodeContactCode(contactCodeString: string): ContactCode {
	const bin = toByteArray(contactCodeString);
	const [device_pubkey_bytes, inbox_nonce_bytes, share_intent] = decode(bin);
	const device_pubkey = bytesToHex(device_pubkey_bytes);
	const inbox_nonce = inbox_nonce_bytes ? bytesToHex(inbox_nonce_bytes) : undefined;
	return { device_pubkey, share_intent, inbox_nonce };
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
