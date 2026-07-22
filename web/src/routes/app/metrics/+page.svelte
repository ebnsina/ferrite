<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { Card, StatusPill, Icon } from '$lib/ui';
	import { listJobs, getUsage } from '$lib/api/endpoints';
	import { ApiError } from '$lib/api/client';
	import type { Job, JobState, Usage } from '$lib/api/types';
	import { bytes, timeAgo } from '$lib/format';
	import { Analytics01Icon, RefreshIcon } from '@hugeicons/core-free-icons';

	const ACTIVE: JobState[] = ['queued', 'probing', 'transcoding', 'packaging', 'uploading'];
	const ALL_STATES: JobState[] = [
		'queued',
		'probing',
		'transcoding',
		'packaging',
		'uploading',
		'completed',
		'failed'
	];

	let jobs = $state<Job[]>([]);
	let usage = $state<Usage | null>(null);
	let error = $state<string | null>(null);
	let loading = $state(true);
	let timer: ReturnType<typeof setInterval>;

	async function refresh() {
		try {
			[jobs, usage] = await Promise.all([listJobs(), getUsage()]);
			error = null;
		} catch (e) {
			error = e instanceof ApiError ? e.message : 'Failed to load metrics.';
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		refresh();
		timer = setInterval(refresh, 5000); // live-ish; cheap tenant-scoped queries
	});
	onDestroy(() => clearInterval(timer));

	const counts = $derived(
		ALL_STATES.reduce(
			(acc, s) => ({ ...acc, [s]: jobs.filter((j) => j.state === s).length }),
			{} as Record<JobState, number>
		)
	);
	const total = $derived(jobs.length);
	const active = $derived(jobs.filter((j) => ACTIVE.includes(j.state)).length);
	const completed = $derived(counts.completed ?? 0);
	const failed = $derived(counts.failed ?? 0);
	const successRate = $derived(
		completed + failed === 0 ? null : (completed / (completed + failed)) * 100
	);
	const maxCount = $derived(Math.max(1, ...ALL_STATES.map((s) => counts[s] ?? 0)));

	// "Logs": most recent job activity, newest first.
	const activity = $derived(
		[...jobs]
			.sort(
				(a, b) =>
					new Date(b.finished_at ?? b.queued_at).getTime() -
					new Date(a.finished_at ?? a.queued_at).getTime()
			)
			.slice(0, 20)
	);

	const stats = $derived([
		{ label: 'Total jobs', value: String(total) },
		{ label: 'Active', value: String(active) },
		{ label: 'Completed', value: String(completed) },
		{ label: 'Success rate', value: successRate === null ? '—' : `${successRate.toFixed(0)}%` }
	]);
</script>

<div class="mx-auto max-w-5xl">
	<div class="mb-8 flex items-center justify-between">
		<div>
			<h1 class="flex items-center gap-2 text-2xl font-semibold tracking-tight">
				<span class="text-accent"><Icon icon={Analytics01Icon} size={24} /></span> Metrics
			</h1>
			<p class="mt-1 text-sm text-muted">Live transcode activity for your workspace.</p>
		</div>
		<span class="flex items-center gap-1.5 text-xs text-muted">
			<Icon icon={RefreshIcon} size={13} /> auto-refresh 5s
		</span>
	</div>

	{#if error}
		<div class="mb-4 rounded-lg border border-danger/30 bg-danger/10 px-4 py-3 text-sm text-danger">
			{error}
		</div>
	{/if}

	<div class="mb-6 grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
		{#each stats as s (s.label)}
			<Card>
				<p class="text-sm text-muted">{s.label}</p>
				<p class="mono mt-1 text-2xl font-semibold">{s.value}</p>
			</Card>
		{/each}
	</div>

	<div class="grid gap-6 lg:grid-cols-2">
		<!-- State breakdown -->
		<Card>
			<h2 class="mb-4 text-sm font-medium text-muted">Jobs by state</h2>
			{#if total === 0}
				<p class="py-8 text-center text-sm text-muted">No jobs yet.</p>
			{:else}
				<div class="flex flex-col gap-2.5">
					{#each ALL_STATES as s (s)}
						<div class="flex items-center gap-3">
							<span class="mono w-24 shrink-0 text-xs text-muted">{s}</span>
							<div class="h-2 flex-1 overflow-hidden rounded-full bg-surface-2">
								<div
									class="h-full rounded-full bg-accent transition-all"
									style={`width: ${((counts[s] ?? 0) / maxCount) * 100}%`}
								></div>
							</div>
							<span class="mono w-8 shrink-0 text-right text-xs">{counts[s] ?? 0}</span>
						</div>
					{/each}
				</div>
			{/if}
		</Card>

		<!-- Usage -->
		<Card>
			<h2 class="mb-4 text-sm font-medium text-muted">Usage this month</h2>
			{#if usage}
				<div class="grid grid-cols-2 gap-4">
					<div>
						<p class="text-xs text-muted">Transcoded</p>
						<p class="mono mt-1 text-lg font-semibold">{usage.minutes.toFixed(1)} min</p>
					</div>
					<div>
						<p class="text-xs text-muted">Storage</p>
						<p class="mono mt-1 text-lg font-semibold">{bytes(usage.storage_bytes)}</p>
					</div>
					<div>
						<p class="text-xs text-muted">Failed jobs</p>
						<p class="mono mt-1 text-lg font-semibold">{failed}</p>
					</div>
					<div>
						<p class="text-xs text-muted">Est. cost</p>
						<p class="mono mt-1 text-lg font-semibold text-accent">${usage.cost.total.toFixed(2)}</p>
					</div>
				</div>
			{:else}
				<p class="py-8 text-center text-sm text-muted">Loading…</p>
			{/if}
		</Card>
	</div>

	<!-- Activity log -->
	<Card class="mt-6">
		<h2 class="mb-4 text-sm font-medium text-muted">Recent activity</h2>
		{#if loading && activity.length === 0}
			<p class="py-8 text-center text-sm text-muted">Loading…</p>
		{:else if activity.length === 0}
			<p class="py-8 text-center text-sm text-muted">No activity yet.</p>
		{:else}
			<div class="divide-y divide-border">
				{#each activity as job (job.id)}
					<a
						href={`/app/jobs/${job.id}`}
						class="flex items-center gap-4 py-2.5 transition-colors hover:bg-surface-2"
					>
						<code class="mono w-20 shrink-0 truncate text-xs text-muted">{job.id.slice(0, 8)}</code>
						<span class="flex-1 text-xs text-muted"
							>{timeAgo(job.finished_at ?? job.queued_at)}</span
						>
						{#if job.error}<span class="truncate text-xs text-danger">{job.error}</span>{/if}
						<StatusPill state={job.state} />
					</a>
				{/each}
			</div>
		{/if}
	</Card>
</div>
