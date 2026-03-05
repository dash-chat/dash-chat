import Emittery from 'emittery';

import type { ISettingsClient, Settings } from '../settings/settings-client';

export class MockSettingsClient implements ISettingsClient {
	private settings: Settings = {
		qr_color: null,
		color_scheme: null,
		local_mailbox_enabled: false,
	};

	private emitter = new Emittery<{ updated: Settings }>();

	async getSettings(): Promise<Settings> {
		return { ...this.settings };
	}

	async setSetting(key: string, value: unknown): Promise<void> {
		(this.settings as unknown as Record<string, unknown>)[key] = value;
		this.emitter.emit('updated', { ...this.settings });
	}

	onSettingsUpdated(handler: (settings: Settings) => void): () => void {
		return this.emitter.on('updated', handler);
	}
}
