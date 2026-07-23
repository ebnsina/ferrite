<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Icon, Sheet } from '$lib/ui';
	import { listMembers, inviteMember, createApiKey } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { session } from '$lib/api/session.svelte';
	import { inviteSchema, apiKeySchema, validate } from '$lib/schemas';
	import type { Member, MemberInvited } from '$lib/api/types';
	import { timeAgo, nameFromEmail } from '$lib/format';
	import {
		UserGroupIcon,
		Copy01Icon,
		Tick01Icon,
		UserAdd01Icon,
		KeyframeIcon
	} from '@hugeicons/core-free-icons';

	const isOwner = $derived(session.user?.role === 'owner');

	let members = $state<Member[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let copied = $state<string | null>(null);

	// Invite sheet
	let inviteOpen = $state(false);
	let inviteEmail = $state('');
	let inviteRole = $state<'admin' | 'member'>('member');
	let inviting = $state(false);
	let inviteErrors = $state<Record<string, string>>({});
	let invited = $state<MemberInvited | null>(null);

	// API-key sheet
	let keyOpen = $state(false);
	let keyName = $state('');
	let creatingKey = $state(false);
	let keyError = $state<string | null>(null);
	let newKey = $state<string | null>(null);

	async function load() {
		loading = true;
		try {
			members = await listMembers();
			error = null;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load team.';
		} finally {
			loading = false;
		}
	}
	onMount(load);

	function openInvite() {
		inviteEmail = '';
		inviteRole = 'member';
		inviteErrors = {};
		invited = null;
		inviteOpen = true;
	}

	async function invite() {
		inviteErrors = {};
		const v = validate(inviteSchema, { email: inviteEmail, role: inviteRole });
		if (!v.ok) return (inviteErrors = v.errors);
		inviting = true;
		try {
			invited = await inviteMember(v.data.email, v.data.role);
			await load();
		} catch (e) {
			inviteErrors = { email: e instanceof ApiError ? e.message : 'Could not send invite.' };
		} finally {
			inviting = false;
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
		} catch (e) {
			keyError = e instanceof ApiError ? e.message : 'Could not create key.';
		} finally {
			creatingKey = false;
		}
	}

	function copy(value: string, tag: string) {
		navigator.clipboard.writeText(value);
		copied = tag;
		setTimeout(() => (copied = null), 1500);
	}

	const inputCls =
		'w-full rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent';
</script>

<div class="mx-auto max-w-4xl">
	<div class="mb-8 flex flex-wrap items-center justify-between gap-3">
		<div>
			<h1 class="text-2xl font-semibold tracking-tight">Team</h1>
			<p class="mt-1 text-sm text-muted">Members and access for {session.tenant?.name}.</p>
		</div>
		{#if isOwner}
			<div class="flex gap-2">
				<Button variant="secondary" onclick={openKey}>
					<Icon icon={KeyframeIcon} size={16} /> API key
				</Button>
				<Button onclick={openInvite}>
					<Icon icon={UserAdd01Icon} size={16} /> Invite member
				</Button>
			</div>
		{/if}
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
	{/if}

	<!-- Members -->
	<Card>
		<h2 class="mb-4 text-sm font-medium text-muted">Members</h2>
		{#if loading}
			<p class="py-6 text-center text-sm text-muted">Loading…</p>
		{:else if members.length === 0}
			<div class="flex flex-col items-center py-10 text-center">
				<span class="mb-3 text-muted"><Icon icon={UserGroupIcon} size={28} /></span>
				<p class="text-sm text-muted">No members yet.</p>
			</div>
		{:else}
			<div class="divide-y divide-border">
				{#each members as m (m.id)}
					<div class="flex items-center justify-between py-3">
						<div class="flex min-w-0 items-center gap-3">
							<span
								class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-accent-soft text-sm font-semibold text-accent"
								>{(m.name || m.email).charAt(0).toUpperCase()}</span
							>
							<div class="min-w-0">
								<p class="truncate text-sm font-medium">{m.name || nameFromEmail(m.email)}</p>
								<p class="truncate text-xs text-muted">{m.email} · joined {timeAgo(m.created_at)}</p>
							</div>
						</div>
						<span
							class="mono rounded-full border border-border bg-surface-2 px-2 py-0.5 text-xs text-muted"
							>{m.role}</span
						>
					</div>
				{/each}
			</div>
		{/if}
	</Card>
</div>

<!-- Invite sheet -->
<Sheet
	open={inviteOpen}
	onclose={() => (inviteOpen = false)}
	title="Invite a member"
	description="Creates the account with a one-time temporary password to share."
>
	{#if invited}
		<div class="rounded-lg border border-success/30 bg-success/10 p-4">
			<p class="text-sm font-medium">Invited {invited.member.email}</p>
			<p class="mt-1 mb-3 text-xs text-muted">Share this temporary password — shown only once.</p>
			<div class="flex items-center gap-2 rounded-lg border border-border bg-surface-2 p-2.5">
				<code class="mono flex-1 truncate text-sm">{invited.temp_password}</code>
				<button
					onclick={() => copy(invited!.temp_password, 'invite')}
					class="text-muted hover:text-fg"
					aria-label="Copy temporary password"
				>
					<Icon icon={copied === 'invite' ? Tick01Icon : Copy01Icon} size={16} />
				</button>
			</div>
		</div>
	{:else}
		<div class="flex flex-col gap-4">
			<div>
				<label for="inv-email" class="mb-1.5 block text-xs font-medium text-muted">Email</label>
				<input id="inv-email" bind:value={inviteEmail} type="email" placeholder="teammate@company.com" class={inputCls} />
				{#if inviteErrors.email}<p class="mt-1.5 text-sm text-danger">{inviteErrors.email}</p>{/if}
			</div>
			<div>
				<label for="inv-role" class="mb-1.5 block text-xs font-medium text-muted">Role</label>
				<select id="inv-role" bind:value={inviteRole} class={inputCls}>
					<option value="member">Member</option>
					<option value="admin">Admin</option>
				</select>
			</div>
		</div>
	{/if}

	{#snippet footer()}
		{#if invited}
			<Button class="w-full" onclick={() => (inviteOpen = false)}>Done</Button>
		{:else}
			<div class="flex justify-end gap-2">
				<Button variant="secondary" onclick={() => (inviteOpen = false)}>Cancel</Button>
				<Button disabled={inviting} onclick={invite}>{inviting ? 'Inviting…' : 'Send invite'}</Button>
			</div>
		{/if}
	{/snippet}
</Sheet>

<!-- API key sheet -->
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
				<button onclick={() => copy(newKey!, 'key')} class="text-muted hover:text-fg" aria-label="Copy API key">
					<Icon icon={copied === 'key' ? Tick01Icon : Copy01Icon} size={16} />
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
