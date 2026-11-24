<script lang="ts">
	import internetSvg from '$lib/assets/connection-status/internet.svg?raw';
	import internetPlusSvg from '$lib/assets/connection-status/internet-plus.svg?raw';
	import isolationSvg from '$lib/assets/connection-status/isolation.svg?raw';
	import localHubSvg from '$lib/assets/connection-status/local-hub.svg?raw';
	import localHubPlusSvg from '$lib/assets/connection-status/local-hub-plus.svg?raw';
	import p2pSvg from '$lib/assets/connection-status/p2p.svg?raw';
	import seekingSvg from '$lib/assets/connection-status/seeking.svg?raw';
	import type { ConnectionStatus } from './connection-status';

	let { status }: { status: ConnectionStatus } = $props();

	function glyph(status: ConnectionStatus): string {
		switch (status.status) {
			case 'cloud':
				return status.hasAnotherRoute ? internetPlusSvg : internetSvg;
			case 'local':
				return status.hasAnotherRoute ? localHubPlusSvg : localHubSvg;
			case 'p2p':
				return p2pSvg;
			case 'disconnected':
				return isolationSvg;
			case 'pending':
				return seekingSvg;
		}
	}
</script>

<span class="block size-6 *:size-full">
	{@html glyph(status)}
</span>
