import { reactive, relay, signal } from 'signalium';

import { DevicesStore } from '../devices/devices-store';
import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { AgentId, DeviceId, TopicId } from '../p2panda/types';
import { personalTopicFor } from '../topics';
import { AnnouncementPayload, Payload } from '../types';
import { IContactsClient, Profile } from './contacts-client';

export interface ContactRequest {
	profile: Profile;
	agentId: AgentId;
	timestamp: number;
	topicId: TopicId;
}

/**
 * An outgoing contact request we've sent by scanning a QR code, before the
 * owner's ack has arrived. Keyed on the owner's device pubkey, since we don't
 * yet know their agent id or profile.
 */
export interface OutgoingContactRequest {
	devicePubkey: DeviceId;
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

	private reportVersion = signal(0);

	/**
	 * Whether the given contact has already been reported. Reported state lives
	 * in the local store (not the synced logs), so it's tracked with a version
	 * signal that `reportContact` bumps rather than derived from the op logs.
	 */
	contactReported = reactive(async (agentId: AgentId) => {
		void this.reportVersion.value;
		return await this.client.isContactReported(agentId);
	});

	/**
	 * Report a contact. Resolves with every mailbox it has been reported to so
	 * far — an empty array means no mailbox could be reached, so the report
	 * hasn't been delivered and the caller should tell the user to retry.
	 */
	async reportContact(agentId: AgentId): Promise<string[]> {
		const mailboxes = await this.client.reportContact(agentId);
		this.reportVersion.value++;
		return mailboxes;
	}

	/**
	 * Outgoing contact requests we've sent but whose ack hasn't arrived yet.
	 * A pending marker is dropped once its device pubkey resolves to an
	 * established contact (the ack was processed and the `AddContact` marker
	 * created a real chat that supersedes the placeholder).
	 */
	outgoingPendingRequests = reactive(async () => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		const latestByDevice: Record<DeviceId, number> = {};
		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (op.body?.payload?.type !== 'PendingContactRequest') continue;
				const { device_pubkey } = op.body.payload.payload;
				const existing = latestByDevice[device_pubkey];
				if (existing === undefined || op.header.timestamp > existing) {
					latestByDevice[device_pubkey] = op.header.timestamp;
				}
			}
		}

		const devices = Object.keys(latestByDevice);
		const resolved = await Promise.all(
			devices.map(device => this.client.agentForDevice(device)),
		);

		const pending: OutgoingContactRequest[] = [];
		for (let i = 0; i < devices.length; i++) {
			if (resolved[i] !== undefined) continue;
			pending.push({
				devicePubkey: devices[i],
				timestamp: latestByDevice[devices[i]],
			});
		}
		return pending;
	});

	contactAddedTimestamp = reactive(async (agentId: AgentId) => {
		const myDeviceGroupTopic = await this.devicesStore.myDeviceGroupTopic();

		for (const [_, ops] of Object.entries(myDeviceGroupTopic)) {
			for (const op of ops) {
				if (
					op.body?.payload?.type === 'AddContact' &&
					op.body.payload.payload.agent_id === agentId
				) {
					return op.header.timestamp;
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
				if (!existingTimestamp || op.header.timestamp > existingTimestamp) {
					rejected[agentId] = op.header.timestamp;
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

	profilesForAllContacts = reactive(async () => {
		const contacts = await this.contactsAgentIds();

		const profiles = await Promise.all(
			contacts.map(contact => this.profiles(contact)),
		);

		const profilesWithContacts: Array<[AgentId, Profile]> = contacts
			.map(
				(contact, i) =>
					[contact, profiles[i]] as [AgentId, Profile | undefined],
			)
			.filter((pair): pair is [AgentId, Profile] => !!pair[1]);

		return profilesWithContacts;
	});
}
