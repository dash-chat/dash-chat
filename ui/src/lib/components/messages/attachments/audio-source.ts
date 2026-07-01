import { blobUrl } from '$lib/utils/media';
import type { VoiceNote } from 'dash-chat-stores';

export interface LoadedAudio {
	data: Uint8Array;
	mimeType: string;
}

/** Fetch a voice note's bytes for playback; undefined on failure.
 * WebKitGTK's `<audio>` can't load our custom `irohblob://` scheme (its media
 * pipeline bypasses the webview's scheme handler), so callers play the bytes
 * from a `blob:` URL instead. */
export async function loadVoiceAudio(
	voice: VoiceNote,
): Promise<LoadedAudio | undefined> {
	try {
		const res = await fetch(blobUrl(voice.hash));
		if (!res.ok) return undefined;
		return {
			data: new Uint8Array(await res.arrayBuffer()),
			mimeType: voice.mime_type,
		};
	} catch {
		return undefined;
	}
}
