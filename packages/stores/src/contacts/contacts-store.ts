import { reactive, relay } from 'signalium';

import { DevicesStore } from '../devices/devices-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, Hash, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import { AnnouncementPayload, ChatId, Payload } from '../types';
import { IContactsClient, Profile } from './contacts-client';

export interface ContactRequest {
	profile: Profile;
	agentId: AgentId;
	devicePubkey: DeviceId;
	chatId: ChatId;
	timestamp: number;
	topicId: TopicId;
}

/**
 * An outgoing contact request we've sent by scanning a QR code. Keyed on the
 * owner's device pubkey, since we don't know their agent id at scan time.
 */
export interface OutgoingContactRequest {
	devicePubkey: DeviceId;
	chatId: ChatId;
	timestamp: number;
	profileName: string;
}

export interface Contact {
	agentId: AgentId;
	chatId: ChatId;
	addedTimestamp: number;
}

export interface ContactWithProfile {
	contact: Contact;
	profile: Profile;
}

/** One filed report against a contact, as recorded in the device group log. */
export interface ContactReport {
	timestamp: number;
	author: DeviceId;
	mailboxCount: number;
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

	/** The established contacts, keyed by agent id. */
	contacts = reactive(async (): Promise<Record<AgentId, Contact>> => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const contacts: Record<AgentId, Contact> = {};

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (op.body?.payload?.type === 'AddContact') {
					const { agent_id, direct_chat_topic_id } = op.body.payload.payload;
					contacts[agent_id] = {
						agentId: agent_id,
						chatId: direct_chat_topic_id,
						addedTimestamp: op.header.timestamp,
					};
				}
			}
		}

		return contacts;
	});

	contactsAgentIds = reactive(async () => {
		return Object.keys(await this.contacts());
	});

	blockedContactAgentIds = reactive(async () => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const latestByAgent: Record<
			AgentId,
			{ blocked: boolean; timestamp: number }
		> = {};
		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				const payload = op.body?.payload;
				if (payload?.type !== 'BlockAgent' && payload?.type !== 'UnblockAgent')
					continue;
				const agentId = payload.payload;
				const existing = latestByAgent[agentId];
				if (!existing || op.header.timestamp > existing.timestamp) {
					latestByAgent[agentId] = {
						blocked: payload.type === 'BlockAgent',
						timestamp: op.header.timestamp,
					};
				}
			}
		}

		const blocked = new Set<AgentId>();
		for (const [agentId, v] of Object.entries(latestByAgent)) {
			if (v.blocked) blocked.add(agentId as AgentId);
		}
		return blocked;
	});

	/**
	 * Every report this device group has filed against `agentId`, keyed by the
	 * hash of the `ReportContact` operation. Reporting stays available after a
	 * report, so an agent can have any number of these.
	 */
	reports = reactive(async (agentId: AgentId) => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const reports: Record<Hash, ContactReport> = {};
		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				const payload = op.body?.payload;
				if (payload?.type !== 'ReportContact') continue;
				if (payload.payload.agent_id !== agentId) continue;
				reports[op.hash] = {
					timestamp: op.header.timestamp,
					author: op.header.verifying_key,
					mailboxCount: payload.payload.mailbox_ids.length,
				};
			}
		}
		return reports;
	});

	reportContact = async (agentId: AgentId) => {
		await this.client.reportContact(agentId);
	};

	/** Every block/unblock operation for `agentId`, keyed by operation hash. */
	blockHistory = reactive(async (agentId: AgentId) => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const events: Record<
			Hash,
			{ blocked: boolean; timestamp: number; author: DeviceId }
		> = {};
		for (const ops of Object.values(myDeviceGroupTopic)) {
			for (const op of ops) {
				const payload = op.body?.payload;
				if (payload?.type !== 'BlockAgent' && payload?.type !== 'UnblockAgent')
					continue;
				if (payload.payload !== agentId) continue;
				events[op.hash] = {
					blocked: payload.type === 'BlockAgent',
					timestamp: op.header.timestamp,
					author: op.header.verifying_key,
				};
			}
		}
		return events;
	});

	isBlocked = reactive(async (agentId: AgentId) => {
		const blocked = await this.blockedContactAgentIds();

		return blocked.has(agentId);
	});

	/**
	 * Outgoing contact requests we've sent, latest per device pubkey. Kept
	 * even after the contact is established: the QR name recorded here is the
	 * only name we have for the peer until their profile syncs.
	 */
	outgoingContactRequests = reactive(async () => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const latestByDevice: Record<DeviceId, OutgoingContactRequest> = {};
		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (op.body?.payload?.type !== 'PendingContactRequest') continue;
				const { device_pubkey, profile_name, direct_chat_topic_id } =
					op.body.payload.payload;
				const existing = latestByDevice[device_pubkey];
				if (
					existing === undefined ||
					op.header.timestamp > existing.timestamp
				) {
					latestByDevice[device_pubkey] = {
						devicePubkey: device_pubkey,
						chatId: direct_chat_topic_id,
						timestamp: op.header.timestamp,
						profileName: profile_name ?? '',
					};
				}
			}
		}

		return Object.values(latestByDevice);
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
				if (!existingTimestamp || op.header.timestamp > existingTimestamp) {
					rejected[agentId] = op.header.timestamp;
				}
			}
		}

		return rejected;
	});

	private directChatId = reactive(async (devicePubkey: DeviceId) =>
		this.client.directChatId(devicePubkey),
	);

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

		for (let i = 0; i < allLogs.length; i++) {
			const topicId = activeInboxTopics[i];
			const log = allLogs[i];
			for (const operations of Object.values(log)) {
				for (const operation of operations) {
					if (operation.body?.type !== 'Inbox') continue;
					if (operation.body.payload.type !== 'ContactRequest') continue;
					const { profile, agent_id } = operation.body.payload.payload;
					if (!agent_id) continue;
					const agentId = agent_id;

					// We have already accepted this contact request
					if (contacts.includes(agentId)) continue;

					// Time-based rejection: only filter if request was made BEFORE rejection
					const rejectionTimestamp = rejectedMap[agentId];
					if (
						rejectionTimestamp &&
						operation.header.timestamp < rejectionTimestamp
					)
						continue;

					contactRequests.push({
						profile,
						agentId,
						devicePubkey: operation.header.verifying_key,
						chatId: await this.directChatId(operation.header.verifying_key),
						topicId,
						timestamp: operation.header.timestamp,
					});
				}
			}
		}

		return contactRequests;
	});

	/** Get a profile from inbox contact requests for a given agent, regardless of acceptance status. */
	private inboxProfile = reactive(async (agentId: AgentId) => {
		const activeInboxTopics = await this.activeInboxTopics();

		const allLogs = await Promise.all(
			activeInboxTopics.map(topicId =>
				this.logsStore.logsForAllAuthors(topicId),
			),
		);

		let latest: { timestamp: number; profile: Profile } | undefined;

		for (const log of allLogs) {
			for (const operations of Object.values(log)) {
				for (const operation of operations) {
					if (operation.body?.type !== 'Inbox') continue;
					if (operation.body.payload.type !== 'ContactRequest') continue;
					const { profile, agent_id } = operation.body.payload.payload;
					if (agent_id !== agentId) continue;
					const ts = operation.header.timestamp;
					if (!latest || ts > latest.timestamp) {
						latest = { timestamp: ts, profile };
					}
				}
			}
		}

		return latest?.profile;
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
				(l.body!.payload as AnnouncementPayload).payload as Profile,
			]);

		const descendantSortedOperations = setProfiles.sort(
			(o1, o2) => o2[0] - o1[0],
		);
		const lastOperation = descendantSortedOperations[0];

		if (!lastOperation) {
			// Fallback: use profile from inbox contact request if personal topic hasn't synced
			return await this.inboxProfile(agentId);
		}

		const profile: Profile = lastOperation[1];
		return profile;
	});

	profilesForUnblockedContacts = reactive(
		async (): Promise<ContactWithProfile[]> => {
			const [contacts, blocked] = await Promise.all([
				this.contacts(),
				this.blockedContactAgentIds(),
			]);
			const unblocked = Object.values(contacts).filter(
				contact => !blocked.has(contact.agentId),
			);

			const profiles = await Promise.all(
				unblocked.map(contact => this.profiles(contact.agentId)),
			);

			return unblocked
				.map((contact, i) => ({ contact, profile: profiles[i] }))
				.filter(
					(entry): entry is ContactWithProfile => entry.profile !== undefined,
				);
		},
	);
}
