import type { IContactsClient, Profile } from '../contacts/contacts-client';
import type { AgentId, DeviceId, TopicId } from '../p2panda/types';
import type { ContactCode, InboxTopic } from '../types';
import { personalTopicFor } from '../topics';
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

	async setProfile(profile: Profile): Promise<void> {
		await this.logsClient.create(personalTopicFor(this.agentId), {
			type: 'Announcements',
			payload: { type: 'SetProfile', payload: profile },
		});
	}

	async createContactCode(): Promise<ContactCode> {
		return {
			device_pubkey: this.deviceId,
			agent_id: this.agentId,
			inbox_topic: this.inboxTopics[0]
				? { topic: this.inboxTopics[0], expires_at: Date.now() + 86400000 }
				: undefined,
			share_intent: 'AddContact',
		};
	}

	async activeInboxTopics(): Promise<TopicId[]> {
		return this.inboxTopics;
	}

	async addContact(contactCode: ContactCode): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'AddContact', payload: contactCode },
		});
	}

	async rejectContactRequest(agentId: AgentId): Promise<void> {
		await this.logsClient.create(this.deviceGroupTopicId, {
			type: 'DeviceGroupPayload',
			payload: { type: 'RejectContactRequest', payload: agentId },
		});
	}
}
