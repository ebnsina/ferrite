<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Button, Icon, Sheet, toast } from '$lib/ui';
	import { listMembers, inviteMember, updateMemberRole, removeMember } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError } from '$lib/humanize';
	import { session } from '$lib/api/session.svelte';
	import { inviteSchema, validate } from '$lib/schemas';
	import type { Member, MemberInvited } from '$lib/api/types';
	import { timeAgo, nameFromEmail } from '$lib/format';
	import { UsersThree, Copy, Check, UserPlus, Trash } from 'phosphor-svelte';

	const isOwner = $derived(session.user?.role === 'owner');

	let members = $state<Member[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let copied = $state<string | null>(null);
	let rowBusy = $state<string | null>(null);
	let confirmingRemove = $state<string | null>(null);

	// Invite sheet
	let inviteOpen = $state(false);
	let inviteEmail = $state('');
	let inviteRole = $state<'admin' | 'member'>('member');
	let inviting = $state(false);
	let inviteErrors = $state<Record<string, string>>({});
	let invited = $state<MemberInvited | null>(null);

	async function load() {
		loading = true;
		try {
			members = await listMembers();
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your team.');
		} finally {
			loading = false;
		}
	}
	onMount(load);

	async function changeRole(m: Member, role: 'admin' | 'member') {
		if (m.role === role) return;
		rowBusy = m.id;
		try {
			await updateMemberRole(m.id, role);
			toast.success(`${nameFromEmail(m.email)} is now ${role === 'admin' ? 'an admin' : 'a member'}.`);
			await load();
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not update role.'));
		} finally {
			rowBusy = null;
		}
	}

	async function remove(m: Member) {
		rowBusy = m.id;
		try {
			await removeMember(m.id);
			confirmingRemove = null;
			toast.success(`${nameFromEmail(m.email)} was removed from the team.`);
			await load();
		} catch (e) {
			toast.error(humanizeError(e instanceof ApiError ? e.message : null, 'Could not remove member.'));
		} finally {
			rowBusy = null;
		}
	}

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
			toast.success(`Invite ready for ${v.data.email}.`);
			await load();
		} catch (e) {
			inviteErrors = {
				email: humanizeError(e instanceof ApiError ? e.message : null, 'Could not send invite.')
			};
		} finally {
			inviting = false;
		}
	}

	function copy(value: string, tag: string) {
		navigator.clipboard.writeText(value);
		copied = tag;
		toast.success('Copied to clipboard.');
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
			<Button onclick={openInvite}>
				<Icon icon={UserPlus} size={16} /> Invite member
			</Button>
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
				<span class="mb-3 text-muted"><Icon icon={UsersThree} size={28} /></span>
				<p class="text-sm text-muted">No members yet.</p>
			</div>
		{:else}
			<div class="divide-y divide-border">
				{#each members as m (m.id)}
					<div class="flex items-center justify-between gap-3 py-3">
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

						{#if isOwner && m.role !== 'owner'}
							<div class="flex shrink-0 items-center gap-2">
								<select
									value={m.role}
									disabled={rowBusy === m.id}
									onchange={(e) => changeRole(m, e.currentTarget.value as 'admin' | 'member')}
									class="rounded-lg border border-border bg-surface-2 px-2 py-1 text-xs outline-none focus:border-accent"
								>
									<option value="member">Member</option>
									<option value="admin">Admin</option>
								</select>
								{#if confirmingRemove === m.id}
									<button
										onclick={() => remove(m)}
										disabled={rowBusy === m.id}
										class="rounded-lg bg-danger/10 px-2 py-1 text-xs font-medium text-danger hover:bg-danger/20"
										>Confirm</button
									>
									<button
										onclick={() => (confirmingRemove = null)}
										class="text-xs text-muted hover:text-fg">Cancel</button
									>
								{:else}
									<button
										onclick={() => (confirmingRemove = m.id)}
										aria-label="Remove member"
										class="rounded-lg p-1.5 text-muted transition-colors hover:bg-danger/10 hover:text-danger"
									>
										<Icon icon={Trash} size={16} />
									</button>
								{/if}
							</div>
						{:else}
							<span
								class="mono rounded-full border border-border bg-surface-2 px-2 py-0.5 text-xs text-muted"
								>{m.role}</span
							>
						{/if}
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
					<Icon icon={copied === 'invite' ? Check : Copy} size={16} />
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
