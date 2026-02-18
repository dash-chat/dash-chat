import { reactive } from 'signalium';

import { LogsStore } from '../p2panda/logs-store';
import { SimplifiedOperation } from '../p2panda/simplified-types';
import { PublicKey } from '../p2panda/types';
import { DeviceGroupPayload, Payload } from '../types';
import { IDevicesClient } from './devices-client';

export class DevicesStore {
	constructor(
		protected logsStore: LogsStore<Payload>,
		public client: IDevicesClient,
	) {}

	myDeviceGroupTopicId = reactive(
		async () => await this.client.myDeviceGroupTopicId(),
	);

	myDeviceGroupTopic = reactive(async () => {
		const topicId = await this.myDeviceGroupTopicId();

		const operations = await this.logsStore.logsForAllAuthors(topicId);

		return operations as Record<
			PublicKey,
			SimplifiedOperation<{
				type: 'DeviceGroupPayload';
				payload: DeviceGroupPayload;
			}>[]
		>;
	});
}
