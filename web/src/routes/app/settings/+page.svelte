<script lang="ts">
	import { Card, Button, Icon } from '$lib/ui';
	import { session } from '$lib/api/session.svelte';
	import { theme } from '$lib/theme.svelte';
	import { nameFromEmail } from '$lib/format';
	import { Sun03Icon, Moon02Icon, Logout01Icon } from '@hugeicons/core-free-icons';

	const initial = $derived((session.user?.email ?? '?').charAt(0).toUpperCase());
	const name = $derived(nameFromEmail(session.user?.email));

	const modes = [
		{ value: 'light' as const, label: 'Light', icon: Sun03Icon },
		{ value: 'dark' as const, label: 'Dark', icon: Moon02Icon }
	];
</script>

<div class="mx-auto max-w-2xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Settings</h1>
		<p class="mt-1 text-sm text-muted">Manage your profile and preferences.</p>
	</div>

	<!-- Profile -->
	<Card class="mb-6">
		<h2 class="mb-4 text-sm font-medium text-muted">Profile</h2>
		<div class="flex items-center gap-4">
			<span
				class="flex h-14 w-14 items-center justify-center rounded-full bg-accent-soft text-xl font-semibold text-accent"
				>{initial}</span
			>
			<div class="min-w-0">
				<p class="truncate text-lg font-semibold">{name}</p>
				<p class="truncate text-sm text-muted">{session.user?.email}</p>
			</div>
		</div>
		<dl class="mt-6 grid gap-px overflow-hidden rounded-lg border border-border bg-border sm:grid-cols-2">
			<div class="bg-surface p-4">
				<dt class="text-xs text-muted">Workspace</dt>
				<dd class="mt-1 text-sm font-medium">{session.tenant?.name}</dd>
			</div>
			<div class="bg-surface p-4">
				<dt class="text-xs text-muted">Role</dt>
				<dd class="mt-1 text-sm font-medium capitalize">{session.user?.role}</dd>
			</div>
		</dl>
	</Card>

	<!-- Appearance -->
	<Card class="mb-6">
		<h2 class="mb-1 text-sm font-medium text-muted">Appearance</h2>
		<p class="mb-4 text-xs text-muted">Choose how Ferrite looks on this device.</p>
		<div class="inline-flex rounded-lg border border-border p-1">
			{#each modes as m (m.value)}
				<button
					onclick={() => theme.set(m.value)}
					class={`flex items-center gap-2 rounded-md px-4 py-2 text-sm font-medium transition-colors ${
						theme.mode === m.value
							? 'bg-accent-soft text-accent'
							: 'text-muted hover:text-fg'
					}`}
				>
					<Icon icon={m.icon} size={16} />
					{m.label}
				</button>
			{/each}
		</div>
	</Card>

	<!-- Session -->
	<Card>
		<h2 class="mb-1 text-sm font-medium text-muted">Session</h2>
		<p class="mb-4 text-xs text-muted">Sign out of Ferrite on this device.</p>
		<Button variant="secondary" onclick={() => session.clear()}>
			<Icon icon={Logout01Icon} size={16} /> Sign out
		</Button>
	</Card>
</div>
