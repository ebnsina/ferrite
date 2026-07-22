<script lang="ts">
	import { Icon } from '$lib/ui';
	import { CheckmarkCircle02Icon, ArrowRight01Icon } from '@hugeicons/core-free-icons';

	const tiers = [
		{
			name: 'Hobby',
			price: '$0',
			cadence: '/mo',
			blurb: 'For side projects and evaluation.',
			cta: 'Start free',
			highlight: false,
			features: [
				'1 workspace',
				'CPU transcoding',
				'HLS + DASH output',
				'5 GB storage',
				'Community support'
			]
		},
		{
			name: 'Pro',
			price: '$49',
			cadence: '/mo',
			blurb: 'For teams shipping video to production.',
			cta: 'Get started',
			highlight: true,
			features: [
				'Unlimited team members',
				'Priority fair-queue lane',
				'Live streaming (RTMP + SRT)',
				'500 GB storage',
				'Signed playback URLs',
				'Email support'
			]
		},
		{
			name: 'Scale',
			price: 'Custom',
			cadence: '',
			blurb: 'For high-volume, multi-tenant platforms.',
			cta: 'Contact us',
			highlight: false,
			features: [
				'GPU (NVENC) encoding',
				'Dedicated workers',
				'Custom storage & regions',
				'DRM & advanced security',
				'SLA & priority support'
			]
		}
	];
</script>

<svelte:head>
	<title>Pricing — Ferrite</title>
	<meta name="description" content="Simple pricing for Ferrite video transcoding — from free to scale." />
</svelte:head>

<section class="mx-auto max-w-6xl px-6 py-20">
	<div class="mx-auto max-w-2xl text-center">
		<h1 class="text-4xl font-semibold tracking-tight">Simple, scalable pricing</h1>
		<p class="mt-3 text-muted">
			Start free, upgrade when you grow. Billing is mocked in this demo environment.
		</p>
	</div>

	<div class="mt-14 grid gap-6 lg:grid-cols-3">
		{#each tiers as t (t.name)}
			<div
				class={`flex flex-col rounded-xl border p-6 ${
					t.highlight ? 'border-accent bg-surface ring-1 ring-accent' : 'border-border bg-surface'
				}`}
			>
				<div class="flex items-center justify-between">
					<h2 class="text-lg font-semibold">{t.name}</h2>
					{#if t.highlight}
						<span class="mono rounded-full bg-accent-soft px-2 py-0.5 text-xs text-accent"
							>Popular</span
						>
					{/if}
				</div>
				<p class="mt-1 text-sm text-muted">{t.blurb}</p>
				<div class="mt-5 flex items-end gap-1">
					<span class="text-4xl font-semibold tracking-tight">{t.price}</span>
					{#if t.cadence}<span class="mb-1 text-sm text-muted">{t.cadence}</span>{/if}
				</div>

				<a
					href="/app"
					class={`mt-6 inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors ${
						t.highlight
							? 'bg-accent text-accent-fg hover:opacity-90'
							: 'border border-border hover:bg-surface-2'
					}`}
				>
					{t.cta} <Icon icon={ArrowRight01Icon} size={15} />
				</a>

				<ul class="mt-6 flex flex-col gap-3 border-t border-border pt-6">
					{#each t.features as f (f)}
						<li class="flex items-start gap-2.5 text-sm">
							<span class="mt-0.5 text-accent"><Icon icon={CheckmarkCircle02Icon} size={16} /></span>
							<span>{f}</span>
						</li>
					{/each}
				</ul>
			</div>
		{/each}
	</div>

	<p class="mx-auto mt-10 max-w-xl text-center text-sm text-muted">
		All plans include adaptive HLS + DASH, the fair multi-tenant queue, and thumbnail generation.
	</p>
</section>
