import { reactive, relay } from 'signalium';

import { DevicesStore } from '../devices/devices-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import { AnnouncementPayload, ChatId, ContactCode, Payload } from '../types';
import { IContactsClient, Profile } from './contacts-client';

export interface ContactRequest {
	profile: Profile;
	code: ContactCode;
	timestamp: number;
}

export class ContactsStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		public devicesStore: DevicesStore,
		public client: IContactsClient,
	) {}

	myAgentId = reactive(async () => await this.client.myAgentId());

	myDeviceId = reactive(async () => await this.client.myDeviceId());

	myProfile = reactive(async () => {
		const myAgentId = await this.myAgentId();

		return await this.profiles(myAgentId);
	});

	private activeInboxTopics = reactive(() =>
		relay<TopicId[]>(state => {
			state.setPromise(this.client.activeInboxTopics());
			const interval = setInterval(() => {
				this.client.activeInboxTopics().then(topics => {
					if (topics.find(topic => !(state.value || []).includes(topic))) {
						state.value = topics;
					}
				});
			}, 1_000);

			return {
				deactivate() {
					clearInterval(interval);
				},
			};
		}),
	);

	contactsAgentIds = reactive(async () => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const contacts: Set<AgentId> = new Set();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (op.body?.payload?.type === 'AddContact') {
					contacts.add(op.body.payload.payload.agent_id);
				}
			}
		}

		return Array.from(contacts);
	});

	contactAddedTimestamp = reactive(async (agentId: AgentId) => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (
					op.body?.payload?.type === 'AddContact' &&
					op.body.payload.payload.agent_id === agentId
				) {
					return op.header.timestamp * 1000;
				}
			}
		}

		return undefined;
	});

	rejectedContactRequests = reactive(async () => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const rejected: Record<AgentId, number> = {};
		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (op.body?.payload?.type !== 'RejectContactRequest') continue;
				const agentId = op.body.payload.payload;

				const existingTimestamp = rejected[agentId];

				// Keep the latest rejection timestamp
				if (
					!existingTimestamp ||
					op.header.timestamp * 1000 > existingTimestamp
				) {
					rejected[agentId] = op.header.timestamp * 1000;
				}
			}
		}

		return rejected;
		});

	contactRequests = reactive(async () => {
		const activeInboxTopics = await this.activeInboxTopics();

		const allLogs = await Promise.all(
			activeInboxTopics.map(topicId =>
				this.logsStore.logsForAllAuthors(topicId),
			),
		);
		const contacts = await this.contactsAgentIds();
		const rejectedMap = await this.rejectedContactRequests();

		const contactRequests: ContactRequest[] = [];

		for (const log of allLogs) {
			for (const operations of Object.values(log)) {
				for (const operation of operations) {
					if (operation.body?.type !== 'Inbox') continue;
					const agentId = operation.body.payload.payload.code.agent_id;

					// We have already accepted this contact request
					if (contacts.includes(agentId)) continue;

					// Time-based rejection: only filter if request was made BEFORE rejection
					const rejectionTimestamp = rejectedMap[agentId];
					if (
						rejectionTimestamp &&
						operation.header.timestamp * 1000 < rejectionTimestamp
					)
						continue;

					contactRequests.push({
						...operation.body.payload.payload,
						timestamp: operation.header.timestamp * 1000,
					});
				}
			}
		}

		return contactRequests;
	});

	profiles = reactive(async (agentId: AgentId) => {
		const topicId = personalTopicFor(agentId);

		const operations = await this.logsStore.logsForAllAuthors(topicId);

		const log: SimplifiedOperation<Payload>[] =
			Object.values(operations)[0] || [];

		const setProfiles: Array<[number, Profile]> = log
			.filter(
				l =>
					l.body?.type === 'Announcements' &&
					l.body.payload.type === 'SetProfile',
			)
			.map(l => [
				l.header.timestamp,
				(l.body!.payload as AnnouncementPayload).payload,
			]);

		const descendantSortedOperations = setProfiles.sort(
			(o1, o2) => o2[0] - o1[0],
		);
		const lastOperation = descendantSortedOperations[0];

		if (!lastOperation) {
			return undefined;
		}

		const profile: Profile = lastOperation[1];
		return profile;
	});

	profilesForAllContacts = reactive(async () => {
		const contacts = await this.contactsAgentIds();

		const profiles = await Promise.all(
			contacts.map(contact => this.profiles(contact)),
		);

		const profilesWithContacts: Array<[AgentId, Profile]> = profiles
			.filter(p => !!p)
			.map((profile, i) => [contacts[i], profile]);

		return profilesWithContacts;
	});
}
