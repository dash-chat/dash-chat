import { blobUrl } from '$lib/utils/media';
import type { VoiceNote } from 'dash-chat-stores';
import { tick } from 'svelte';

export interface LoadedAudio {
	data: Uint8Array;
	mimeType: string;
}

/**
 * Lazily fetches a voice note's bytes on first play and exposes them as a
 * reactive `source` for the `objectUrl` action. WebKitGTK's `<audio>` can't load
 * our custom `irohblob://` scheme (its media pipeline bypasses the webview's
 * scheme handler), so we fetch the bytes and play them from a `blob:` URL.
 */
export class AudioSourceLoader {
	source = $state<LoadedAudio>();
	#loadPromise: Promise<boolean> | undefined;

	constructor(private voice: () => VoiceNote) {}

	/** Resolves true once the bytes are loaded (or already were), false on failure.
	 * Concurrent calls share the same in-flight fetch. */
	ensureLoaded(): Promise<boolean> {
		if (this.source) return Promise.resolve(true);
		if (this.#loadPromise) return this.#loadPromise;
		this.#loadPromise = (async () => {
			const voice = this.voice();
			try {
				const res = await fetch(blobUrl(voice.hash));
				if (!res.ok) return false;
				this.source = {
					data: new Uint8Array(await res.arrayBuffer()),
					mimeType: voice.mime_type,
				};
				await tick(); // let the `objectUrl` action set <audio>.src
				return true;
			} catch {
				return false;
			} finally {
				this.#loadPromise = undefined;
			}
		})();
		return this.#loadPromise;
	}
}
