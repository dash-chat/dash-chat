import { fromByteArray, toByteArray } from 'base64-js';
// @ts-ignore
import { decode, encode } from 'cbor-web';

import { ContactCode, ShareIntent } from '../types';

function hexToBytes(hex: string): Uint8Array {
	return Uint8Array.from(
		(hex.match(/.{1,2}/g) ?? []).map(byte => parseInt(byte, 16)),
	);
}

function bytesToHex(bytes: Uint8Array): string {
	return Array.from(bytes, b => b.toString(16).padStart(2, '0')).join('');
}

function toPaddedBase64(base64: string): string {
	const remainder = base64.length % 4;
	if (remainder === 0) {
		return base64;
	}
	if (remainder === 1) {
		throw new Error('Invalid base64 contact code');
	}
	return base64.padEnd(base64.length + (4 - remainder), '=');
}

// The hex string keys are converted to raw bytes before CBOR encoding.
export function encodeContactCode(contactCode: ContactCode): string {
	const bin = encode([
		hexToBytes(contactCode.device_pubkey),
		hexToBytes(contactCode.inbox_nonce),
		contactCode.share_intent,
	]);
	return fromByteArray(bin).replace(/=+$/, '');
}

export function decodeContactCode(contactCodeString: string): ContactCode {
	const bin = toByteArray(toPaddedBase64(contactCodeString));
	const [device_pubkey_bytes, inbox_nonce_bytes, share_intent] = decode(
		bin,
	) as [Uint8Array, Uint8Array, ShareIntent];
	const device_pubkey = bytesToHex(device_pubkey_bytes);
	const inbox_nonce = bytesToHex(inbox_nonce_bytes);
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
