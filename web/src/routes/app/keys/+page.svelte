<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Icon, Sheet, toast } from '$lib/ui';
	import { listApiKeys, createApiKey, revokeApiKey } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import { session } from '$lib/api/session.svelte';
	import { apiKeySchema, validate } from '$lib/schemas';
	import type { ApiKey } from '$lib/api/types';
	import { timeAgo } from '$lib/format';
	import { Key, Copy, Check, Plus } from 'phosphor-svelte';

	const isOwner = $derived(session.user?.role === 'owner');

	let apiKeys = $state<ApiKey[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let rowBusy = $state<string | null>(null);
	let copied = $state(false);

	let keyOpen = $state(false);
	let keyName = $state('');
	let creatingKey = $state(false);
	let keyError = $state<string | null>(null);
	let newKey = $state<string | null>(null);

	async function load() {
		if (!isOwner) {
			loading = false;
			return;
		}
		loading = true;
		try {
			apiKeys = await listApiKeys();
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your API keys.');
		} finally {
			loading = false;
		}
	}
	onMount(load);

	async function revoke(k: ApiKey) {
		rowBusy = k.id;
		try {
			await revokeApiKey(k.id);
			toast.success('API key revoked.');
			await load();
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not revoke key.'));
		} finally {
			rowBusy = null;
		}
	}

	function openKey() {
		keyName = '';
		keyError = null;
		newKey = null;
		keyOpen = true;
	}

	async function genKey() {
		keyError = null;
		const v = validate(apiKeySchema, { name: keyName });
		if (!v.ok) return (keyError = v.errors.name);
		creatingKey = true;
		try {
			const res = await createApiKey(v.data.name);
			newKey = res.api_key;
			await load();
		} catch (e) {
			keyError = humanizeError(e instanceof ApiError ? e.message : null, 'Could not create key.');
		} finally {
			creatingKey = false;
		}
	}

	function copy(value: string) {
		navigator.clipboard.writeText(value);
		copied = true;
		toast.success('Copied to clipboard.');
		setTimeout(() => (copied = false), 1500);
	}

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';
</script>

<div class="mx-auto max-w-4xl">
	<div class="mb-8 flex flex-wrap items-center justify-between gap-3">
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
				<span class="text-accent"><Icon icon={Key} size={20} /></span> API keys
			</h1>
			<p class="mt-1 text-sm text-muted">Programmatic access for SDKs, CI, and the REST API.</p>
		</div>
		{#if isOwner}
			<Button onclick={openKey}><Icon icon={Plus} size={16} /> New key</Button>
		{/if}
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	{#if !isOwner}
		<Card>
			<p class="py-10 text-center text-sm text-muted">Only the workspace owner can manage API keys.</p>
		</Card>
	{:else}
		<Card>
			{#if loading}
				<p class="py-8 text-center text-sm text-muted">Loading…</p>
			{:else if apiKeys.length === 0}
				<div class="flex flex-col items-center justify-center py-12 text-center">
					<span class="mb-3 text-muted"><Icon icon={Key} size={28} /></span>
					<p class="font-medium">No API keys yet</p>
					<p class="mt-1 text-sm text-muted">Create one to call the Ferrite Stream API programmatically.</p>
				</div>
			{:else}
				<div class="divide-y divide-border">
					{#each apiKeys as k (k.id)}
						<div class="flex items-center justify-between gap-3 py-3">
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">
									{k.name}
									{#if k.revoked}<span class="ml-2 text-xs text-danger">revoked</span>{/if}
								</p>
								<p class="mono truncate text-xs text-muted">
									{k.prefix}… · {k.last_used_at ? `used ${timeAgo(k.last_used_at)}` : 'never used'}
								</p>
							</div>
							{#if !k.revoked}
								<button
									onclick={() => revoke(k)}
									disabled={rowBusy === k.id}
									class="shrink-0 rounded-lg px-2.5 py-1 text-xs font-medium text-danger transition-colors hover:bg-danger/10"
								>Revoke</button>
							{/if}
						</div>
					{/each}
				</div>
			{/if}
		</Card>
	{/if}
</div>

<!-- Create key sheet -->
<Sheet
	open={keyOpen}
	onclose={() => (keyOpen = false)}
	title="Create API key"
	description="For programmatic access — SDKs, CI, and the REST API."
>
	{#if newKey}
		<div class="rounded-lg border border-border bg-surface-2 p-4">
			<p class="mb-2 text-xs text-muted">Copy it now — it won't be shown again.</p>
			<div class="flex items-center gap-2">
				<code class="mono flex-1 truncate text-sm">{newKey}</code>
				<button onclick={() => copy(newKey!)} class="text-muted hover:text-fg" aria-label="Copy API key">
					<Icon icon={copied ? Check : Copy} size={16} />
				</button>
			</div>
		</div>
	{:else}
		<div>
			<label for="key-name" class="mb-1.5 block text-xs font-medium text-muted">Key name</label>
			<input id="key-name" bind:value={keyName} placeholder="e.g. ci-pipeline" class={inputCls} />
			{#if keyError}<p class="mt-1.5 text-sm text-danger">{keyError}</p>{/if}
		</div>
	{/if}

	{#snippet footer()}
		{#if newKey}
			<Button class="w-full" onclick={() => (keyOpen = false)}>Done</Button>
		{:else}
			<div class="flex justify-end gap-2">
				<Button variant="secondary" onclick={() => (keyOpen = false)}>Cancel</Button>
				<Button disabled={creatingKey} onclick={genKey}>{creatingKey ? 'Creating…' : 'Create key'}</Button>
			</div>
		{/if}
	{/snippet}
</Sheet>
