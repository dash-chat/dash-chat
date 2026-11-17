import {
	type DraftMedia,
	type DraftVoiceNote,
	type StagingError,
	UndecodablePhotoError,
	planStaging,
	prepareOutgoingPhoto,
} from '$lib/utils/media';
import type { OutgoingPhoto } from 'dash-chat-stores';

export interface StagingOutcome {
	error?: StagingError;
	undecodablePhotos: number;
}

/** Owns the composer's staged attachments and is the only writer of the
 * draft, so a photo is in it only once it has been prepared for sending. */
export class MediaDraft {
	// Raw: the draft is only ever replaced whole, and its prepared photos go
	// to the backend as they are.
	#media = $state.raw<DraftMedia | undefined>();
	#pendingBatches = $state(0);
	#queue: Promise<void> = Promise.resolve();
	// Bumped by whatever discards the draft, so a batch still being staged
	// stops instead of bringing the draft back.
	#generation = 0;

	constructor(private onAttachmentStaged: () => void) {}

	get media(): DraftMedia | undefined {
		return this.#media;
	}

	get busy(): boolean {
		return this.#pendingBatches > 0;
	}

	/** Stage what the mixing rules allow of `files`, after any batch already
	 * being staged. Photos enter the draft one by one as they are prepared. */
	add(files: File[]): Promise<StagingOutcome> {
		const generation = this.#generation;
		this.#pendingBatches++;
		const outcome = this.#queue
			.then(() => this.#stage(files, generation))
			.finally(() => this.#pendingBatches--);
		this.#queue = outcome.then(
			() => {},
			() => {},
		);
		return outcome;
	}

	removePhoto(index: number) {
		if (this.#media?.kind !== 'photos') return;
		this.#setPhotos(this.#media.items.filter((_, i) => i !== index));
	}

	clear() {
		this.#generation++;
		this.#media = undefined;
	}

	setVoiceNote(voice: DraftVoiceNote) {
		this.#media = { kind: 'voice_note', voice };
	}

	/** Drop what `sent` held, keeping photos staged while it was being sent. */
	discardSent(sent: DraftMedia) {
		if (this.#media?.kind === 'photos' && sent.kind === 'photos') {
			this.#setPhotos(this.#media.items.filter(p => !sent.items.includes(p)));
		} else if (this.#media === sent) {
			this.#media = undefined;
		}
	}

	async #stage(files: File[], generation: number): Promise<StagingOutcome> {
		if (generation !== this.#generation) return { undecodablePhotos: 0 };
		const plan = planStaging(this.#media, files);
		if (plan.file) {
			this.#media = { kind: 'file', file: plan.file };
			this.onAttachmentStaged();
		}
		let undecodablePhotos = 0;
		// One at a time: decoding a batch of full-size photos at once can
		// exhaust the Android webview's memory.
		for (const file of plan.photos) {
			const photo = await prepareOrUndecodable(file);
			if (generation !== this.#generation) return { undecodablePhotos: 0 };
			if (photo) {
				this.#appendPhoto(photo);
			} else {
				undecodablePhotos++;
			}
		}
		return { error: plan.error, undecodablePhotos };
	}

	#appendPhoto(photo: OutgoingPhoto) {
		const staged = this.#media?.kind === 'photos' ? this.#media.items : [];
		this.#media = { kind: 'photos', items: [...staged, photo] };
		this.onAttachmentStaged();
	}

	#setPhotos(photos: OutgoingPhoto[]) {
		this.#media =
			photos.length > 0 ? { kind: 'photos', items: photos } : undefined;
	}
}

async function prepareOrUndecodable(
	file: File,
): Promise<OutgoingPhoto | undefined> {
	try {
		return await prepareOutgoingPhoto(file);
	} catch (e) {
		if (e instanceof UndecodablePhotoError) return undefined;
		throw e;
	}
}
