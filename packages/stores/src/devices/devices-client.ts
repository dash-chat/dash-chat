import { type TopicId } from '../p2panda/types';
import { invokeAfterSetup } from '../utils/invoke-after-setup';

export interface IDevicesClient {
	myDeviceGroupTopicId(): Promise<TopicId>;
}

export class DevicesClient implements IDevicesClient {
	myDeviceGroupTopicId(): Promise<TopicId> {
		return invokeAfterSetup('my_device_group_topic');
	}
}
