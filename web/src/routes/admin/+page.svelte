<script lang="ts">
	import { onMount } from 'svelte';
	import { Card, Icon } from '$lib/ui';
	import {
		getAdminOverview,
		getAdminWaitlist,
		type AdminOverview,
		type WaitlistRow
	} from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { timeAgo } from '$lib/format';
	import {
		UserGroupIcon,
		Film01Icon,
		PlayListIcon,
		Mail01Icon,
		BuildingIcon
	} from '@hugeicons/core-free-icons';

	let overview = $state<AdminOverview | null>(null);
	let waitlist = $state<WaitlistRow[]>([]);
	let error = $state<string | null>(null);

	onMount(async () => {
		try {
			[overview, waitlist] = await Promise.all([getAdminOverview(), getAdminWaitlist()]);
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load admin data.';
		}
	});

	const stats = $derived(
		overview
			? [
					{ label: 'Tenants', value: overview.tenants, icon: BuildingIcon },
					{ label: 'Users', value: overview.users, icon: UserGroupIcon },
					{ label: 'Assets', value: overview.assets, icon: Film01Icon },
					{ label: 'Jobs', value: overview.jobs, icon: PlayListIcon },
					{ label: 'Waitlist', value: overview.waitlist, icon: Mail01Icon }
				]
			: []
	);
</script>

<div class="mx-auto max-w-6xl">
	<div class="mb-8">
		<h1 class="text-2xl font-semibold tracking-tight">Admin console</h1>
		<p class="mt-1 text-sm text-muted">Platform overview and early-access signups.</p>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
	{/if}

	<div class="mb-8 grid grid-cols-2 gap-4 sm:grid-cols-3 lg:grid-cols-5">
		{#each stats as s (s.label)}
			<Card>
				<div class="flex items-center justify-between">
					<div>
						<p class="text-sm text-muted">{s.label}</p>
						<p class="mono mt-1 text-2xl font-semibold">{s.value}</p>
					</div>
					<span class="text-muted"><Icon icon={s.icon} size={20} /></span>
				</div>
			</Card>
		{/each}
	</div>

	<Card>
		<h2 class="mb-4 text-sm font-medium text-muted">Waitlist ({waitlist.length})</h2>
		{#if waitlist.length === 0}
			<p class="py-8 text-center text-sm text-muted">No signups yet.</p>
		{:else}
			<div class="overflow-x-auto">
				<table class="w-full min-w-[720px] text-sm">
					<thead>
						<tr class="border-b border-border text-left text-xs text-muted">
							<th class="py-2 pr-3 font-medium">When</th>
							<th class="py-2 pr-3 font-medium">Name</th>
							<th class="py-2 pr-3 font-medium">Email / WhatsApp</th>
							<th class="py-2 pr-3 font-medium">Country</th>
							<th class="py-2 pr-3 font-medium">Plan</th>
							<th class="py-2 pr-3 font-medium">Volume</th>
							<th class="py-2 pr-3 font-medium">Pay</th>
							<th class="py-2 font-medium">Use case</th>
						</tr>
					</thead>
					<tbody>
						{#each waitlist as w (w.id)}
							<tr class="border-b border-border align-top">
								<td class="py-2.5 pr-3 text-xs whitespace-nowrap text-muted">{timeAgo(w.created_at)}</td>
								<td class="py-2.5 pr-3 font-medium">{w.name}</td>
								<td class="py-2.5 pr-3 text-xs">
									<div class="truncate">{w.email}</div>
									{#if w.whatsapp}<div class="mono text-muted">{w.whatsapp}</div>{/if}
								</td>
								<td class="py-2.5 pr-3 text-xs">{w.country ?? '—'}</td>
								<td class="py-2.5 pr-3 text-xs">{w.plan ?? '—'}</td>
								<td class="py-2.5 pr-3 text-xs">{w.volume ?? '—'}</td>
								<td class="py-2.5 pr-3 text-xs">
									{#if w.payment}<span class="rounded bg-surface-2 px-1.5 py-0.5">{w.payment}</span>{:else}—{/if}
								</td>
								<td class="max-w-[240px] py-2.5 text-xs text-muted">{w.use_case ?? '—'}</td>
							</tr>
						{/each}
					</tbody>
				</table>
			</div>
		{/if}
	</Card>
</div>
