import type { IContactsClient, Profile } from '../contacts/contacts-client';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import type { ContactCode } from '../types';
import { ShareIntent } from '../types';
import type { LocalStorageLogsClient } from './client';

export class MockContactsClient implements IContactsClient {
	constructor(
		private logsClient: LocalStorageLogsClient,
		private agentId: AgentId,
		private deviceId: DeviceId,
		private deviceGroupTopicId: TopicId,
		private inboxTopics: TopicId[],
	) {}

	async myAgentId(): Promise<AgentId> {
		return this.agentId;
	}

	async myDeviceId(): Promise<DeviceId> {
		return this.deviceId;
	}

	async agentForDevice(_devicePubkey: DeviceId): Promise<AgentId | undefined> {
		return undefined;
	}

	async setProfile(profile: Profile): Promise<void> {
		await this.logsClient.create(personalTopicFor(this.agentId), {
			type: 'Announcements',
			payload: { type: 'SetProfile', payload: profile },
		});
	}

	async createContactCode(): Promise<ContactCode> {
		return {
			device_pubkey: this.deviceId,
			share_intent: ShareIntent.AddContact,
			inbox_nonce: Array.from(crypto.getRandomValues(new Uint8Array(8)), b =>
				b.toString(16).padStart(2, '0'),
			).join(''),
		};
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(_contactCode: ContactCode): Promise<void> {}

	async acceptContact(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'AddContact', payload: { agent_id: agentId } },
		});
	}

	async rejectContactRequest(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'RejectContactRequest', payload: agentId },
		});
	}
}
