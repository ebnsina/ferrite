<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Icon } from '$lib/ui';
	import { listMembers, inviteMember, createApiKey } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { session } from '$lib/api/session.svelte';
	import type { Member, MemberInvited } from '$lib/api/types';
	import { timeAgo } from '$lib/format';
	import { UserGroupIcon, Copy01Icon, Tick01Icon, KeyframeIcon } from '@hugeicons/core-free-icons';

	const isOwner = $derived(session.user?.role === 'owner');

	let members = $state<Member[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Invite form
	let inviteEmail = $state('');
	let inviteRole = $state<'admin' | 'member'>('member');
	let inviting = $state(false);
	let inviteError = $state<string | null>(null);
	let invited = $state<MemberInvited | null>(null);

	// API key
	let keyName = $state('');
	let creatingKey = $state(false);
	let newKey = $state<string | null>(null);
	let copied = $state<string | null>(null);

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

	async function invite() {
		if (!inviteEmail.trim() || inviting) return;
		inviting = true;
		inviteError = null;
		invited = null;
		try {
			invited = await inviteMember(inviteEmail.trim(), inviteRole);
			inviteEmail = '';
			await load();
		} catch (e) {
			inviteError = e instanceof ApiError ? e.message : 'Could not send invite.';
		} finally {
			inviting = false;
		}
	}

	async function genKey() {
		if (!keyName.trim() || creatingKey) return;
		creatingKey = true;
		try {
			const res = await createApiKey(keyName.trim());
			newKey = res.api_key;
			keyName = '';
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Could not create key.';
		} finally {
			creatingKey = false;
		}
	}

	function copy(value: string, tag: string) {
		navigator.clipboard.writeText(value);
		copied = tag;
		setTimeout(() => (copied = null), 1500);
	}
</script>

<div class="mx-auto max-w-4xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Team</h1>
		<p class="mt-1 text-sm text-muted">Members and access for {session.tenant?.name}.</p>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
	{/if}

	<!-- Members -->
	<Card class="mb-6">
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
						<div class="min-w-0">
							<p class="truncate text-sm font-medium">{m.email}</p>
							<p class="text-xs text-muted">joined {timeAgo(m.created_at)}</p>
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

	<!-- Invite (owner only) -->
	{#if isOwner}
		<Card class="mb-6">
			<h2 class="mb-1 text-sm font-medium text-muted">Invite a member</h2>
			<p class="mb-4 text-xs text-muted">
				Creates the account with a one-time temporary password to share.
			</p>
			<div class="flex flex-col gap-3 sm:flex-row">
				<input
					bind:value={inviteEmail}
					type="email"
					placeholder="teammate@company.com"
					class="flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				/>
				<select
					bind:value={inviteRole}
					class="rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				>
					<option value="member">Member</option>
					<option value="admin">Admin</option>
				</select>
				<Button disabled={inviting || !inviteEmail.trim()} onclick={invite}>
					{inviting ? 'Inviting…' : 'Invite'}
				</Button>
			</div>
			{#if inviteError}<p class="mt-3 text-sm text-danger">{inviteError}</p>{/if}
			{#if invited}
				<div class="mt-4 rounded-lg border border-success/30 bg-success/10 p-3">
					<p class="text-sm font-medium">Invited {invited.member.email}</p>
					<p class="mb-2 text-xs text-muted">
						Share this temporary password — it won't be shown again.
					</p>
					<div class="flex items-center gap-2 rounded-lg border border-border bg-surface-2 p-2">
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
			{/if}
		</Card>

		<!-- API keys -->
		<Card>
			<h2 class="mb-1 flex items-center gap-2 text-sm font-medium text-muted">
				<Icon icon={KeyframeIcon} size={16} /> API keys
			</h2>
			<p class="mb-4 text-xs text-muted">For programmatic access (SDKs, CI, the REST API).</p>
			<div class="flex flex-col gap-3 sm:flex-row">
				<input
					bind:value={keyName}
					placeholder="Key name (e.g. ci-pipeline)"
					class="flex-1 rounded-lg border border-border bg-surface-2 px-3 py-2 text-sm outline-none focus:border-accent"
				/>
				<Button variant="secondary" disabled={creatingKey || !keyName.trim()} onclick={genKey}>
					{creatingKey ? 'Creating…' : 'Create key'}
				</Button>
			</div>
			{#if newKey}
				<div class="mt-4 rounded-lg border border-border bg-surface-2 p-3">
					<p class="mb-2 text-xs text-muted">Copy it now — it won't be shown again.</p>
					<div class="flex items-center gap-2">
						<code class="mono flex-1 truncate text-sm">{newKey}</code>
						<button
							onclick={() => copy(newKey!, 'key')}
							class="text-muted hover:text-fg"
							aria-label="Copy API key"
						>
							<Icon icon={copied === 'key' ? Tick01Icon : Copy01Icon} size={16} />
						</button>
					</div>
				</div>
			{/if}
		</Card>
	{/if}
</div>
