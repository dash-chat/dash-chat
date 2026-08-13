<script lang="ts">
	import {
		Actions,
		ActionsGroup,
		Dialog,
		DialogButton,
		Preloader,
	} from 'konsta/svelte';
	import { m } from '$lib/paraglide/messages.js';
	import ActionButton from './ActionButton.svelte';
	import { isIos } from '$lib/utils/environment';
	import { showToast } from '$lib/utils/toasts';
	import type { Snippet } from 'svelte';

	type ActionResult = { success: true } | { success: false; error: string };

	export interface Action {
		text: string;
		onClick: () => Promise<ActionResult>;
		destructive?: boolean;
		testid?: string;
	}

	interface Props {
		opened: boolean;
		title: string;
		children: Snippet;
		actions: Action[];
		cancelText?: string;
		cancelTestId?: string;
		onCancel: () => void;
		testid?: string;
	}

	let {
		opened,
		title,
		children,
		actions,
		cancelText = m.cancel(),
		cancelTestId,
		onCancel,
		testid,
	}: Props = $props();

	let loading = $state(false);
	let running = $state<Action | null>(null);

	async function run(action: Action) {
		loading = true;
		running = action;
		try {
			const result = await action.onClick();
			if (!result.success) {
				showToast(result.error, 'error');
			}
		} finally {
			loading = false;
			running = null;
		}
	}
</script>

{#snippet cancelButton()}
	<DialogButton
		onClick={onCancel}
		disabled={loading}
		data-testid={cancelTestId}
	>
		{cancelText}
	</DialogButton>
{/snippet}

{#snippet dialogButton(action: Action)}
	<DialogButton
		class={action.destructive ? '!text-red-500' : ''}
		onClick={() => run(action)}
		disabled={loading}
		data-testid={action.testid}
	>
		{action.text}
		{#if running === action}
			<Preloader class="ms-2 h-4 w-4" />
		{/if}
	</DialogButton>
{/snippet}

{#if isIos}
	<Actions {opened} onBackdropClick={onCancel} data-testid={testid}>
		<ActionsGroup class="flex flex-col gap-3 p-2.5">
			<div class="flex flex-col gap-1 px-3.5 py-2 text-start">
				<span class="text-xl text-black dark:text-white">{title}</span>
				<span class="text-black/60 dark:text-white/60">
					{@render children()}
				</span>
			</div>
			{#each actions as action (action.text)}
				<ActionButton
					destructive={action.destructive}
					onClick={() => run(action)}
					disabled={loading}
					data-testid={action.testid}
				>
					{action.text}
					{#if running === action}
						<Preloader class="ms-2 h-4 w-4" />
					{/if}
				</ActionButton>
			{/each}
			<ActionButton
				onClick={onCancel}
				disabled={loading}
				data-testid={cancelTestId}
			>
				{cancelText}
			</ActionButton>
		</ActionsGroup>
	</Actions>
{:else}
	<Dialog {opened} onBackdropClick={onCancel} {title} data-testid={testid}>
		{@render children()}
		{#snippet buttons()}
			{#if actions.length > 1}
				<div class="flex w-full flex-col items-end gap-2">
					{#each actions as action (action.text)}
						{@render dialogButton(action)}
					{/each}
					{@render cancelButton()}
				</div>
			{:else}
				{@render cancelButton()}
				{#each actions as action (action.text)}
					{@render dialogButton(action)}
				{/each}
			{/if}
		{/snippet}
	</Dialog>
{/if}
