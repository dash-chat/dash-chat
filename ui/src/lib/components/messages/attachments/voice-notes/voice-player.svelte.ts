import { blobUrl } from '$lib/utils/media';
import type { VoiceNote } from 'dash-chat-stores';

// Only one voice note plays at a time: starting a player pauses whichever was
// already playing.
let playing: VoicePlayer | undefined;

/** Playback of a single voice note, shared by the play button and the waveform
 * so both drive the same `<audio>` rather than tracking their own. */
export class VoicePlayer {
	paused = $state(true);
	currentTime = $state(0);
	loading = $state(false);

	#audio: HTMLAudioElement | undefined;
	#objectUrl: string | undefined;
	#loaded = false;
	#loadPromise: Promise<boolean> | undefined;
	readonly #voice: VoiceNote;
	readonly #onError: (() => void) | undefined;

	constructor(voice: VoiceNote, onError?: () => void) {
		this.#voice = voice;
		this.#onError = onError;
	}

	/** Prefers the loaded element's duration; the recorded metadata overshoots
	 * and leaves the played region short of the end. */
	get durationSec(): number {
		const d = this.#audio?.duration;
		return d && isFinite(d) && d > 0 ? d : this.#voice.duration_ms / 1000;
	}

	/** Binds the `<audio>` element; returns a teardown to run on unmount. */
	attach(audio: HTMLAudioElement): () => void {
		this.#audio = audio;
		const onPlay = () => {
			if (playing && playing !== this) playing.#audio?.pause();
			playing = this;
			this.paused = false;
		};
		const onPause = () => {
			if (playing === this) playing = undefined;
			this.paused = true;
			this.#sync();
		};
		const onEnded = () => {
			if (playing === this) playing = undefined;
			this.paused = true;
			audio.currentTime = 0;
			this.#sync();
		};
		audio.addEventListener('play', onPlay);
		audio.addEventListener('pause', onPause);
		audio.addEventListener('timeupdate', this.#sync);
		audio.addEventListener('ended', onEnded);
		return () => {
			audio.removeEventListener('play', onPlay);
			audio.removeEventListener('pause', onPause);
			audio.removeEventListener('timeupdate', this.#sync);
			audio.removeEventListener('ended', onEnded);
			if (playing === this) playing = undefined;
			if (this.#objectUrl) URL.revokeObjectURL(this.#objectUrl);
		};
	}

	async toggle(): Promise<void> {
		const audio = this.#audio;
		if (!audio || this.loading) return;
		if (!audio.paused) {
			audio.pause();
			return;
		}
		if (!(await this.#ensureLoaded())) return;
		try {
			await audio.play();
		} catch {
			this.#onError?.();
		}
	}

	async seekTo(fraction: number): Promise<void> {
		if (!(await this.#ensureLoaded()) || !this.#audio) return;
		this.#audio.currentTime =
			Math.max(0, Math.min(1, fraction)) * this.durationSec;
		this.#sync();
	}

	async seekBy(deltaSec: number): Promise<void> {
		if (!(await this.#ensureLoaded()) || !this.#audio) return;
		this.#audio.currentTime = Math.max(
			0,
			Math.min(this.durationSec, this.#audio.currentTime + deltaSec),
		);
		this.#sync();
	}

	// WebKitGTK's `<audio>` can't load our `irohblob://` scheme — its media
	// pipeline bypasses the webview's scheme handler — so bytes are fetched on
	// first play and set as an object URL, with concurrent callers sharing it.
	#ensureLoaded(): Promise<boolean> {
		if (this.#loaded) return Promise.resolve(true);
		if (!this.#loadPromise) {
			this.#loadPromise = this.#load().finally(() => {
				this.#loadPromise = undefined;
			});
		}
		return this.#loadPromise;
	}

	async #load(): Promise<boolean> {
		this.loading = true;
		try {
			const source = await this.#fetchAudio();
			if (!source) {
				this.#onError?.();
				return false;
			}
			if (!this.#audio) return false;
			this.#objectUrl = URL.createObjectURL(
				new Blob([source.data as BlobPart], { type: source.mimeType }),
			);
			this.#audio.src = this.#objectUrl;
			this.#loaded = true;
			return true;
		} finally {
			this.loading = false;
		}
	}

	async #fetchAudio(): Promise<
		{ data: Uint8Array; mimeType: string } | undefined
	> {
		try {
			const res = await fetch(blobUrl(this.#voice.hash));
			if (!res.ok) return undefined;
			return {
				data: new Uint8Array(await res.arrayBuffer()),
				mimeType: this.#voice.mime_type,
			};
		} catch {
			return undefined;
		}
	}

	#sync = (): void => {
		if (this.#audio) this.currentTime = this.#audio.currentTime;
	};
}
