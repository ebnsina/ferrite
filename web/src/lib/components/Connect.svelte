<script lang="ts">
	import { Button, Card, Icon } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { createTenant } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { FlashIcon, Copy01Icon, Tick01Icon } from '@hugeicons/core-free-icons';

	let name = $state('');
	let creating = $state(false);
	let error = $state<string | null>(null);
	let newKey = $state<string | null>(null);
	let copied = $state(false);

	// Manual-key path for returning users.
	let manualKey = $state('');

	async function create() {
		if (!name.trim()) return;
		creating = true;
		error = null;
		try {
			const res = await createTenant(name.trim());
			newKey = res.api_key; // show once before activating
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not create workspace.';
		} finally {
			creating = false;
		}
	}

	function copyKey() {
		if (!newKey) return;
		navigator.clipboard.writeText(newKey);
		copied = true;
		setTimeout(() => (copied = false), 1500);
	}

	function useManual() {
		if (manualKey.trim()) session.set(manualKey.trim());
	}
</script>

<div class="flex min-h-screen items-center justify-center px-6">
	<Card class="w-full max-w-md">
		<div class="mb-6 flex items-center gap-2">
			<span class="text-accent"><Icon icon={FlashIcon} size={22} /></span>
			<span class="text-xl font-semibold tracking-tight">Ferrite</span>
		</div>

		{#if newKey}
			<h1 class="mb-1 text-lg font-semibold">Save your API key</h1>
			<p class="mb-4 text-sm text-muted">Shown once. Store it somewhere safe.</p>
			<div class="mb-4 flex items-center gap-2 rounded-lg border border-border bg-surface-2 p-3">
				<code class="mono flex-1 truncate text-sm">{newKey}</code>
				<button onclick={copyKey} class="text-muted hover:text-fg" aria-label="Copy key">
					{#if copied}<Icon icon={Tick01Icon} size={16} />{:else}<Icon icon={Copy01Icon} size={16} />{/if}
				</button>
			</div>
			<Button class="w-full" onclick={() => session.set(newKey!)}>Continue to dashboard</Button>
		{:else}
			<h1 class="mb-1 text-lg font-semibold">Create a workspace</h1>
			<p class="mb-4 text-sm text-muted">We'll issue you an API key to get started.</p>
			<input
				bind:value={name}
				placeholder="Workspace name"
				class="mb-3 w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				onkeydown={(e) => e.key === 'Enter' && create()}
			/>
			{#if error}<p class="mb-3 text-sm text-danger">{error}</p>{/if}
			<Button class="w-full" disabled={creating || !name.trim()} onclick={create}>
				{creating ? 'Creating…' : 'Create workspace'}
			</Button>

			<div class="my-4 flex items-center gap-3 text-xs text-muted">
				<span class="h-px flex-1 bg-border"></span>have a key?<span class="h-px flex-1 bg-border"></span>
			</div>
			<div class="flex gap-2">
				<input
					bind:value={manualKey}
					placeholder="frt_…"
					class="mono flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				/>
				<Button variant="secondary" onclick={useManual}>Use</Button>
			</div>
		{/if}
	</Card>
</div>
