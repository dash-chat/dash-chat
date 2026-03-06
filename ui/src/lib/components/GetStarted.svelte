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
		bg: string;
		border: string;
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
			bg: 'bg-[#F6EDE0] dark:bg-amber-900/20',
			border: 'border-[#EBE0CF] dark:border-amber-800/15',
		},
		{
			id: 'add-photo',
			label: () => m.addPhoto(),
			icon: mdiCamera,
			href: '/settings/profile/edit-photo',
			bg: 'bg-[#DEE5D6] dark:bg-[#2A2E20]/20',
			border: 'border-[#D0D9C5] dark:border-[#2A2E20]/15',
		},
		{
			id: 'chat-color',
			label: () => m.chatColor(),
			icon: mdiPalette,
			href: '/settings/appearance',
			bg: 'bg-[#F6EDE0] dark:bg-amber-900/20',
			border: 'border-[#EBE0CF] dark:border-amber-800/15',
		},
		{
			id: 'new-group',
			label: () => m.newGroup(),
			icon: mdiAccountMultiplePlus,
			href: '/new-group',
			bg: 'bg-[#DEE5D6] dark:bg-[#2A2E20]/20',
			border: 'border-[#D0D9C5] dark:border-[#2A2E20]/15',
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
		<p class="mb-3 text-lg font-bold">{m.getStarted()}</p>
		<div class="flex gap-3 overflow-x-auto pb-1">
			{#each visibleCards as card}
				<div
					class="relative w-44 shrink-0 rounded-[20px] border {card.bg} {card.border}"
					data-testid="get-started-{card.id}"
				>
					<a
						href={card.href}
						class="flex flex-col items-center px-5 pb-5 pt-8"
					>
						<wa-icon src={wrapPathInSvg(card.icon)} style="font-size: 28px">
						</wa-icon>
						<span class="mt-2 text-center text-sm font-semibold">{card.label()}</span>
					</a>
					<button
						class="absolute right-2 top-2 z-10 p-1 text-black/40 dark:text-white/40"
						data-testid="get-started-dismiss-{card.id}"
						onclick={() => dismiss(card.id)}
					>
						<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 20px"></wa-icon>
					</button>
				</div>
			{/each}
		</div>
	</div>
{/if}
