import {
	MAX_STAGED_PHOTOS,
	UndecodablePhotoError,
	prepareOutgoingPhoto,
} from '$lib/utils/media';
import type { OutgoingPhoto } from 'dash-chat-stores';
import { beforeEach, describe, expect, it, vi } from 'vitest';

import { MediaDraft } from './media-draft.svelte';

vi.mock('$lib/utils/media', async importOriginal => ({
	...(await importOriginal<typeof import('$lib/utils/media')>()),
	prepareOutgoingPhoto: vi.fn(),
}));

const prepare = vi.mocked(prepareOutgoingPhoto);

function photoFile(name: string): File {
	return new File(['x'], `${name}.jpg`, { type: 'image/jpeg' });
}

function textFile(name: string): File {
	return new File(['x'], `${name}.txt`, { type: 'text/plain' });
}

function prepared(file: File): OutgoingPhoto {
	return {
		data: new Uint8Array([1]),
		name: file.name,
		mime_type: file.type,
		width: 1,
		height: 1,
	};
}

function photoFiles(count: number): File[] {
	return Array.from({ length: count }, (_, i) => photoFile(`photo-${i}`));
}

function stagedNames(draft: MediaDraft): string[] {
	return draft.media?.kind === 'photos'
		? draft.media.items.map(photo => photo.name)
		: [];
}

describe('MediaDraft', () => {
	beforeEach(() => {
		prepare.mockReset();
		prepare.mockImplementation(async file => {
			if (file.name.startsWith('damaged')) throw new UndecodablePhotoError();
			return prepared(file);
		});
	});

	it('stages decodable photos and counts the damaged ones', async () => {
		const draft = new MediaDraft(() => {});
		const outcome = await draft.add([
			photoFile('damaged-1'),
			photoFile('good'),
			photoFile('damaged-2'),
		]);
		expect(outcome).toEqual({ error: undefined, undecodablePhotos: 2 });
		expect(stagedNames(draft)).toEqual(['good.jpg']);
		expect(draft.busy).toBe(false);
		expect(draft.pendingPhotos).toBe(0);
	});

	it('stages nothing when the only photo is damaged', async () => {
		const draft = new MediaDraft(() => {});
		const outcome = await draft.add([photoFile('damaged')]);
		expect(outcome.undecodablePhotos).toBe(1);
		expect(draft.media).toBeUndefined();
	});

	it('signals the start of a batch before its photos are prepared', async () => {
		const pendingAtStart: number[] = [];
		const draft: MediaDraft = new MediaDraft(() =>
			pendingAtStart.push(draft.pendingPhotos),
		);
		await draft.add(photoFiles(3));
		expect(pendingAtStart).toEqual([3]);
	});

	it('prepares nothing for a batch the mixing rules reject', async () => {
		const draft = new MediaDraft(() => {});
		const outcome = await draft.add([photoFile('good'), textFile('notes')]);
		expect(outcome).toEqual({ error: 'filesWithPhotos', undecodablePhotos: 0 });
		expect(prepare).not.toHaveBeenCalled();
		expect(draft.media).toBeUndefined();
	});

	it('caps photos across two queued batches', async () => {
		const draft = new MediaDraft(() => {});
		const first = draft.add(photoFiles(MAX_STAGED_PHOTOS - 2));
		const second = draft.add(photoFiles(5));
		expect((await first).error).toBeUndefined();
		expect((await second).error).toBe('tooMany');
		expect(stagedNames(draft)).toHaveLength(MAX_STAGED_PHOTOS);
		expect(prepare).toHaveBeenCalledTimes(MAX_STAGED_PHOTOS);
	});

	it('stops a batch, and reports nothing for it, once the draft is cleared', async () => {
		const draft = new MediaDraft(() => {});
		let finishFirstPhoto = () => {};
		prepare.mockImplementationOnce(
			file =>
				new Promise(resolve => {
					finishFirstPhoto = () => resolve(prepared(file));
				}),
		);
		const staging = draft.add(photoFiles(MAX_STAGED_PHOTOS + 1));
		await vi.waitFor(() => expect(prepare).toHaveBeenCalledTimes(1));

		draft.clear();
		expect(draft.busy).toBe(false);
		expect(draft.pendingPhotos).toBe(0);
		finishFirstPhoto();

		expect(await staging).toEqual({ undecodablePhotos: 0 });
		expect(draft.media).toBeUndefined();
		expect(prepare).toHaveBeenCalledTimes(1);
		expect(draft.busy).toBe(false);
	});

	it('rejects when preparing a photo fails for another reason', async () => {
		const draft = new MediaDraft(() => {});
		prepare.mockRejectedValueOnce(new Error('unreadable'));
		await expect(draft.add([photoFile('gone')])).rejects.toThrow('unreadable');
		expect(draft.busy).toBe(false);
		expect(draft.pendingPhotos).toBe(0);
	});

	it('keeps photos staged while the draft was being sent', async () => {
		const draft = new MediaDraft(() => {});
		await draft.add([photoFile('sent')]);
		const sent = draft.media;
		await draft.add([photoFile('late')]);
		if (sent) draft.discardSent(sent);
		expect(stagedNames(draft)).toEqual(['late.jpg']);
	});

	it('cancels a batch being staged when a voice note replaces the draft', async () => {
		const draft = new MediaDraft(() => {});
		const staging = draft.add(photoFiles(2));
		draft.setVoiceNote({
			bytes: new Uint8Array(),
			mimeType: 'audio/ogg',
			durationMs: 1000,
			waveform: new Uint8Array(),
		});
		await staging;
		expect(draft.media?.kind).toBe('voice_note');
	});
});
