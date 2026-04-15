import type { IDevicesClient } from '../devices/devices-client';
import type { TopicId } from '../p2panda/types';

export class MockDevicesClient implements IDevicesClient {
	constructor(private deviceGroupTopicId: TopicId) {}

	async myDeviceGroupTopicId(): Promise<TopicId> {
		return this.deviceGroupTopicId;
	}
}
