import type { IDevicesClient, TopicId } from 'dash-chat-stores';

export class MockDevicesClient implements IDevicesClient {
	constructor(private deviceGroupTopicId: TopicId) {}

	async myDeviceGroupTopicId(): Promise<TopicId> {
		return this.deviceGroupTopicId;
	}
}
