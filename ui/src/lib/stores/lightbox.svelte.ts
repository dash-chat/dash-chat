import type { Photo } from 'dash-chat-stores';

export interface LightboxContent {
	photos: Photo[];
	index: number;
	senderName: string;
	timestamp: number;
}

let content = $state<LightboxContent | undefined>(undefined);
let triggerEl: HTMLElement | undefined;

/**
 * Global photo viewer state. `open` remembers the triggering element so
 * `close` can restore focus to it.
 */
export const lightbox = {
	get content() {
		return content;
	},
	open(c: LightboxContent, trigger?: HTMLElement) {
		content = c;
		triggerEl = trigger;
	},
	close() {
		content = undefined;
		const el = triggerEl;
		triggerEl = undefined;
		el?.focus();
	},
	select(index: number) {
		if (!content) return;
		const clamped = Math.max(0, Math.min(content.photos.length - 1, index));
		content = { ...content, index: clamped };
	},
	next() {
		if (content) this.select(content.index + 1);
	},
	prev() {
		if (content) this.select(content.index - 1);
	},
};
