<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, StatusPill, Icon } from '$lib/ui';
	import { listJobs } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import { humanizeError, humanizeJobError } from '$lib/humanize';
	import type { Job, JobState } from '$lib/api/types';
	import { timeAgo } from '$lib/format';
	import { PulseIcon, RefreshIcon } from '@hugeicons/core-free-icons';

	type Filter = 'all' | 'completed' | 'failed' | 'active';
	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];

	let jobs = $state<Job[]>([]);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let filter = $state<Filter>('all');
	let timer: ReturnType<typeof setInterval>;

	async function load() {
		try {
			jobs = await listJobs();
			error = null;
		} catch (e) {
			error = humanizeError(e instanceof ApiError ? e.message : null, 'We couldn’t load your activity.');
		} finally {
			loading = false;
		}
	}
	onMount(() => {
		load();
		timer = setInterval(load, 5000);
	});
	onDestroy(() => clearInterval(timer));

	const sorted = $derived(
		[...jobs].sort(
			(a, b) =>
				new Date(b.finished_at ?? b.queued_at).getTime() -
				new Date(a.finished_at ?? a.queued_at).getTime()
		)
	);
	const shown = $derived(
		sorted.filter((j) => {
			if (filter === 'all') return true;
			if (filter === 'active') return ACTIVE.includes(j.state);
			return j.state === filter;
		})
	);

	const filters: { key: Filter; label: string }[] = [
		{ key: 'all', label: 'All' },
		{ key: 'active', label: 'Active' },
		{ key: 'completed', label: 'Completed' },
		{ key: 'failed', label: 'Failed' }
	];
</script>

<div class="mx-auto max-w-4xl">
	<div class="mb-8 flex flex-wrap items-center justify-between gap-4">
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
				<span class="text-accent"><Icon icon={PulseIcon} size={22} /></span> Activity
			</h1>
			<p class="mt-1 text-sm text-muted">Every job across your workspace, newest first.</p>
		</div>
		<span class="flex items-center gap-1.5 text-xs text-muted">
			<Icon icon={RefreshIcon} size={13} /> auto-refresh
		</span>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">{error}</div>
	{/if}

	<!-- Filters -->
	<div class="mb-4 flex items-center gap-1 rounded-lg border border-border bg-surface-2 p-0.5 text-sm">
		{#each filters as f (f.key)}
			<button
				onclick={() => (filter = f.key)}
				class={`rounded-md px-3 py-1.5 transition-colors ${filter === f.key ? 'bg-surface font-medium text-fg shadow-sm' : 'text-muted hover:text-fg'}`}
			>{f.label}</button>
		{/each}
	</div>

	<Card>
		{#if loading && jobs.length === 0}
			<p class="py-10 text-center text-sm text-muted">Loading…</p>
		{:else if shown.length === 0}
			<p class="py-12 text-center text-sm text-muted">
				{jobs.length === 0 ? 'No activity yet.' : 'Nothing matches this filter.'}
			</p>
		{:else}
			<div class="divide-y divide-border">
				{#each shown as job (job.id)}
					<a href={`/app/jobs/${job.id}`} class="flex items-center gap-4 py-3 transition-colors hover:bg-surface-2">
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<span class="w-24 shrink-0 text-xs text-muted">{timeAgo(job.finished_at ?? job.queued_at)}</span>
						<span class="min-w-0 flex-1">
							{#if job.error}
								<span class="block truncate text-xs text-danger">{humanizeJobError(job.error)}</span>
							{/if}
						</span>
						<StatusPill state={job.state} />
					</a>
				{/each}
			</div>
		{/if}
	</Card>
</div>
