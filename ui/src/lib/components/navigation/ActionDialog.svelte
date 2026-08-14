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

	type ActionResult =
		| { success: true }
		| { success: false; error: string; cause?: unknown };

	export interface Action {
		text: string;
		onClick: () => Promise<ActionResult>;
		destructive?: boolean;
		testid?: string;
	}

	interface Props {
		opened: boolean;
		title: string;
		description?: string;
		actions: Action[];
		cancelText?: string;
		cancelTestId?: string;
		onCancel: () => void;
		testid?: string;
	}

	let {
		opened,
		title,
		description,
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
				showToast(
					result.error,
					result.cause === undefined ? 'error' : 'unexpected',
					result.cause,
				);
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
		<ActionsGroup
			class="flex flex-col gap-2 !bg-white p-2.5 dark:!bg-neutral-900"
		>
			<div
				class="flex flex-col px-2.5 pb-2 pt-1 text-start text-[17px] leading-[22px] text-black dark:text-white"
			>
				<span class:font-semibold={description !== undefined}>{title}</span>
				{#if description !== undefined}
					<span>{description}</span>
				{/if}
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
		{#if description !== undefined}
			<span>{description}</span>
		{/if}
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
