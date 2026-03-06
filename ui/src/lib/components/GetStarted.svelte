<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import {
		mdiAccountMultiplePlus,
		mdiAccountPlus,
		mdiCamera,
		mdiClose,
		mdiPalette,
	} from '@mdi/js';
	import type { ContactsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';

	interface Card {
		id: string;
		label: () => string;
		icon: string;
		href: string;
		color: string;
		hidden?: boolean;
	}

	const DISMISSED_KEY = 'get-started-dismissed';

	let { visible = $bindable(true) }: { visible?: boolean } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);
	let hasAvatar = $state(false);
	$effect(() => {
		const p = $myProfile;
		p.then((profile) => {
			hasAvatar = !!profile?.avatar;
		});
	});

	const allCards: Card[] = [
		{
			id: 'add-contact',
			label: () => m.addContact(),
			icon: mdiAccountPlus,
			href: '/new-message/add-contact',
			color: 'bg-[#F7F0E4] dark:bg-amber-900/20',
		},
		{
			id: 'add-photo',
			label: () => m.addPhoto(),
			icon: mdiCamera,
			href: '/settings/profile/edit-photo',
			color: 'bg-[#EDEEE6] dark:bg-[#2A2E20]/20',
		},
		{
			id: 'chat-color',
			label: () => m.chatColor(),
			icon: mdiPalette,
			href: '/settings/appearance',
			color: 'bg-[#F7F0E4] dark:bg-amber-900/20',
		},
		{
			id: 'new-group',
			label: () => m.newGroup(),
			icon: mdiAccountMultiplePlus,
			href: '/new-group',
			color: 'bg-[#EDEEE6] dark:bg-[#2A2E20]/20',
			hidden: true,
		},
	];

	function getDismissed(): string[] {
		try {
			return JSON.parse(localStorage.getItem(DISMISSED_KEY) || '[]');
		} catch {
			return [];
		}
	}

	let dismissed = $state(getDismissed());

	let visibleCards = $derived(
		allCards.filter((c) => {
			if (c.hidden) return false;
			if (dismissed.includes(c.id)) return false;
			if (c.id === 'add-photo' && hasAvatar) return false;
			return true;
		}),
	);
	$effect(() => {
		visible = visibleCards.length > 0;
	});

	function dismiss(id: string) {
		dismissed = [...dismissed, id];
		try {
			localStorage.setItem(DISMISSED_KEY, JSON.stringify(dismissed));
		} catch (e) {
			console.error('Failed to persist dismissed cards:', e);
		}
	}
</script>

{#if visibleCards.length > 0}
	<div class="px-4 pb-4">
		<p class="mb-2 text-base font-semibold">{m.getStarted()}</p>
		<div class="flex gap-3 overflow-x-auto pb-1">
			{#each visibleCards as card}
				<div
					class="relative w-40 shrink-0 rounded-2xl border border-black/5 dark:border-white/10 {card.color}"
					data-testid="get-started-{card.id}"
				>
					<a
						href={card.href}
						class="flex flex-col items-center px-5 pb-5 pt-8"
					>
						<wa-icon src={wrapPathInSvg(card.icon)} style="font-size: 28px; opacity: 0.8">
						</wa-icon>
						<span class="mt-2 text-center text-sm font-medium">{card.label()}</span>
					</a>
					<button
						class="absolute right-1.5 top-1.5 z-10 p-1 text-black/25 dark:text-white/25"
						data-testid="get-started-dismiss-{card.id}"
						onclick={() => dismiss(card.id)}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 16px"></wa-icon>
					</button>
				</div>
			{/each}
		</div>
	</div>
{/if}
