import { invoke } from '@tauri-apps/api/core';

import { type TopicId } from '../p2panda/types';

export interface IDevicesClient {
	myDeviceGroupTopicId(): Promise<TopicId>;
}

export class DevicesClient implements IDevicesClient {
	myDeviceGroupTopicId(): Promise<TopicId> {
		return invoke('my_device_group_topic');
	}
}
