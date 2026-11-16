import { pushState } from '$app/navigation';
import { page } from '$app/state';

export function showAddContactInContentPanel() {
	if (page.state.contentPanel === 'add-contact') return;
	pushState('', { ...page.state, contentPanel: 'add-contact' });
}
