<script lang="ts">
	import AddContactPanel from '$lib/components/layout/AddContactPanel.svelte';
	import { isWideScreen } from '$lib/stores/screen.svelte';
</script>

<AddContactPanel showBack={!isWideScreen.value} />
