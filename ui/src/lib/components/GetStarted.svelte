<script lang="ts">
	import '@awesome.me/webawesome/dist/components/icon/icon.js';
	import { m } from '$lib/paraglide/messages.js';
	import { wrapPathInSvg } from '$lib/utils/icon';
	import { goto } from '$app/navigation';
	import {
		mdiAccountMultiplePlus,
		mdiAccountPlus,
		mdiCamera,
		mdiClose,
		mdiPalette,
	} from '@mdi/js';
	import type { ContactsStore, ChatsStore } from 'dash-chat-stores';
	import { getContext } from 'svelte';
	import { useReactivePromise } from '$lib/stores/use-signal';
	import { useVisibleChatSummaries } from '$lib/stores/visible-chats';
	import { useTheme } from 'konsta/svelte';

	type CardColor = 'warm' | 'sage';

	interface Card {
		id: string;
		label: () => string;
		icon: string;
		href: string;
		tone: CardColor;
		hidden?: boolean;
	}

	const theme = $derived(useTheme());

	// Pixel-exact colors extracted from Signal screenshots
	const colors: Record<string, Record<CardColor, string>> = {
		ios: { warm: 'bg-[#F6F2E9]', sage: 'bg-[#E9ECE4]' },
		material: { warm: 'bg-[#F5ECDF]', sage: 'bg-[#DDE4D5]' },
	};
	const darkColors: Record<CardColor, string> = {
		warm: 'dark:bg-amber-900/20',
		sage: 'dark:bg-[#2A2E20]/20',
	};

	const DISMISSED_KEY = 'get-started-dismissed';

	let { visible = $bindable(true) }: { visible?: boolean } = $props();

	const contactsStore: ContactsStore = getContext('contacts-store');
	const chatsStore: ChatsStore = getContext('chats-store');
	const myProfile = useReactivePromise(contactsStore.myProfile);
	const contacts = useReactivePromise(contactsStore.contactsAgentIds);
	const chatSummaries = useVisibleChatSummaries(chatsStore);

	let hasAvatar = $state(false);
	$effect(() => {
		const p = $myProfile;
		p.then(profile => {
			hasAvatar = !!profile?.avatar;
		});
	});

	const allCards: Card[] = [
		{
			id: 'add-contact',
			label: () => m.addContact(),
			icon: mdiAccountPlus,
			href: '/new-message/add-contact',
			tone: 'warm',
		},
		{
			id: 'add-photo',
			label: () => m.addPhoto(),
			icon: mdiCamera,
			href: '/settings/profile/edit-photo',
			tone: 'sage',
		},
		{
			id: 'chat-color',
			label: () => m.chatColor(),
			icon: mdiPalette,
			href: '/settings/appearance?setup=true',
			tone: 'warm',
		},
		{
			id: 'new-group',
			label: () => m.newGroup(),
			icon: mdiAccountMultiplePlus,
			href: '/new-group',
			tone: 'sage',
			hidden: true,
		},
	];

	function cardClasses(tone: CardColor): string {
		const isIos = theme === 'ios';
		const t = isIos ? 'ios' : 'material';
		const bg = `${colors[t][tone]} ${darkColors[tone]}`;
		// iOS Signal cards have a subtle shadow + thin white border; Material has neither
		const border = isIos
			? 'shadow-[0_1px_4px_rgba(0,0,0,0.06)] border border-white/50 dark:border-white/10'
			: '';
		return `${bg} ${border}`;
	}

	function getDismissed(): string[] {
		try {
			return JSON.parse(localStorage.getItem(DISMISSED_KEY) || '[]');
		} catch {
			return [];
		}
	}

	let dismissed = $state(getDismissed());

	let visibleCards = $derived(
		allCards.filter(c => {
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
		if (dismissed.includes(id)) return;
		dismissed = [...dismissed, id];
		try {
			localStorage.setItem(DISMISSED_KEY, JSON.stringify(dismissed));
		} catch (e) {
			console.error('Failed to persist dismissed cards:', e);
		}
	}

	function onCardClick(id: string, href: string) {
		if (id === 'chat-color') {
			dismiss(id);
		}
		goto(href);
	}
</script>

{#await $contacts then contactsList}
	{#await $chatSummaries then chats}
		{#if (contactsList?.length ?? 0) === 0 && (chats?.length ?? 0) === 0 && visibleCards.length > 0}
			<p class="px-4 mb-3 text-lg font-bold pointer-events-auto">
				{m.getStarted()}
			</p>
			<div class="px-4 flex gap-3.5 overflow-x-auto pb-1 pointer-events-auto">
				{#each visibleCards as card}
					<div
						class="relative w-[165px] shrink-0 rounded-[20px] {cardClasses(
							card.tone,
						)}"
						data-testid="get-started-{card.id}"
					>
						<a
							href={card.href}
							onclick={e => {
								e.preventDefault();
								onCardClick(card.id, card.href);
							}}
							class="flex flex-col items-center px-5 pb-5 pt-7"
						>
							<wa-icon src={wrapPathInSvg(card.icon)} style="font-size: 28px">
							</wa-icon>
							<span class="mt-2 text-center text-sm font-semibold"
								>{card.label()}</span
							>
						</a>
						<button
							class="absolute end-2 top-2 z-10 p-1 text-black/40 dark:text-white/40"
							data-testid="get-started-dismiss-{card.id}"
							onclick={() => dismiss(card.id)}
							aria-label={m.close()}
						>
							<wa-icon src={wrapPathInSvg(mdiClose)} style="font-size: 20px"
							></wa-icon>
						</button>
					</div>
				{/each}
			</div>
		{/if}
	{/await}
{/await}
