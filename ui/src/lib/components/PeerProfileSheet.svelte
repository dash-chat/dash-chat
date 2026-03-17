<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import '@awesome.me/webawesome/dist/components/avatar/avatar.js';
	import { m } from '$lib/paraglide/messages.js';
	import { Sheet, Dialog } from 'konsta/svelte';
	import { fullName, type Profile } from 'dash-chat-stores';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { mdiAccount, mdiAccountGroup } from '@mdi/js';
	import { isWideScreen } from '$lib/stores/screen.svelte';
	import Avatar from './profiles/Avatar.svelte';

	interface Props {
		opened: boolean;
		onClose: () => void;
		profile: Profile | null | undefined;
	}

	let { opened, onClose, profile }: Props = $props();
</script>

{#snippet profileContent()}
	{#if profile}
		<Avatar
			image={profile.avatar}
			initials={profile.name.slice(0, 2)}
			style="--size: 200px;"
		/>

		<div class="flex w-full flex-col gap-3 px-2">
			<span class="text-2xl">{m.about()}</span>

			<div class="flex items-center gap-3">
				<wa-icon class="quiet" src={wrapPathInSvg(mdiAccount)}></wa-icon>
				<span class="text-base">{fullName(profile)}</span>
			</div>

			<div class="flex items-center gap-3">
				<wa-icon class="quiet" src={wrapPathInSvg(mdiAccountGroup)}></wa-icon>
				<span class="text-base">{m.noGroupsInCommon()}</span>
			</div>
		</div>
	{/if}
{/snippet}

{#if isWideScreen.value}
	<Dialog {opened} onBackdropClick={onClose}>
		<div class="flex flex-col items-center gap-4">
			{@render profileContent()}
		</div>
	</Dialog>
{:else}
	<Sheet class="pb-safe" {opened} onBackdropClick={onClose}>
		<div class="flex flex-col items-center px-6 pb-6 gap-4">
			<div class="sheet-handle"></div>
			{@render profileContent()}
		</div>
	</Sheet>
{/if}
