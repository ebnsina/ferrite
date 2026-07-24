<script lang="ts">
	import { onMount } from 'svelte';
	import { fade } from 'svelte/transition';
	import { browser } from '$app/environment';
	import { Icon } from '$lib/ui';
	import { reveal } from '$lib/actions/reveal';
	import { detectCurrency, formatPrice, CURRENCY_LIST, CURRENCIES } from '$lib/currency';
	import { SITE } from '$lib/site';
	import { Stack, ShareNetwork, Cpu, Broadcast, LockKey, Image, CloudArrowUp, Queue, Rocket, ArrowRight, CaretDown, CheckCircle, MagicWand, ClosedCaptioning, Scissors, Code, Database, Coins, Package, X, MagnifyingGlass, ShieldCheck, Play } from 'phosphor-svelte';

	function scrollTo(id: string) {
		document.getElementById(id)?.scrollIntoView({ behavior: 'smooth', block: 'start' });
	}

	// Region-aware pricing: detect on the client, allow override, persist choice.
	let currency = $state('USD');
	let currencyOpen = $state(false);
	const cur = $derived(CURRENCIES[currency] ?? CURRENCIES.USD);

	// Live "link expires in" countdown for the privacy bento visual.
	let secondsLeft = $state(899);
	const expiry = $derived(
		`${String(Math.floor(secondsLeft / 60)).padStart(2, '0')}:${String(secondsLeft % 60).padStart(2, '0')}`
	);

	onMount(() => {
		const saved = localStorage.getItem('ferrite.currency');
		currency = saved && CURRENCIES[saved] ? saved : detectCurrency();
		const t = setInterval(() => (secondsLeft = secondsLeft > 0 ? secondsLeft - 1 : 899), 1000);
		return () => clearInterval(t);
	});
	$effect(() => {
		if (browser) localStorage.setItem('ferrite.currency', currency);
	});
	function pickCurrency(code: string) {
		currency = code;
		currencyOpen = false;
	}

	// Categories of teams Ferrite Stream is built for — not customer names.
	const clients = [
		'Creator platforms',
		'Media teams',
		'Course builders',
		'Live events',
		'Marketplaces',
		'Internal video'
	];

	// Plain-language explainers for non-technical readers: what it is + why it matters.
	const benefits = [
		{
			kind: 'adaptive',
			icon: Stack,
			eyebrow: 'Smooth playback',
			title: 'Plays perfectly on any connection',
			plain:
				'Ferrite Stream automatically makes several versions of your video at different qualities. The viewer’s player picks the best one for their internet speed, moment to moment.',
			benefit: 'No buffering, no spinning wheels — on a laptop, a phone, or a smart TV.'
		},
		{
			kind: 'fair',
			icon: ShareNetwork,
			eyebrow: 'Fair for everyone',
			title: 'Every customer gets their turn',
			plain:
				'When lots of videos are uploaded at once, Ferrite Stream shares the processing evenly instead of working through one big pile first.',
			benefit: 'One customer uploading 10,000 videos never makes everyone else wait in line.'
		},
		{
			kind: 'live',
			icon: Broadcast,
			eyebrow: 'Live, then on-demand',
			title: 'Go live and keep the replay',
			plain:
				'Broadcast a live stream through the same platform, and Ferrite Stream quietly records it while it happens.',
			benefit: 'The moment your event ends, an on-demand version is ready to watch.'
		},
		{
			kind: 'private',
			icon: LockKey,
			eyebrow: 'Private by default',
			title: 'Your videos stay yours',
			plain:
				'Instead of public links, videos are served through signed links that expire — so they can’t be copied around or scraped.',
			benefit: 'Only the people you allow can press play.'
		}
	];

	const features = [
		{
			icon: Stack,
			title: 'Adaptive HLS + DASH',
			body: 'One CMAF encode packages both HLS and DASH with shared fMP4 segments — ready for any player.'
		},
		{
			icon: MagicWand,
			title: 'AI vertical shorts',
			body: 'Auto-find highlights from the transcript, reframe to 9:16, and burn in captions. Provider-agnostic — cloud or fully local.'
		},
		{
			icon: ClosedCaptioning,
			title: 'Auto-captions',
			body: 'Transcribe speech to WebVTT with whisper.cpp locally or any OpenAI-compatible endpoint. Nothing leaves your box unless you want it to.'
		},
		{
			icon: Cpu,
			title: 'Per-title encoding',
			body: 'Content-aware bitrate ladders tailored to each source — cut egress on simple videos without touching quality on complex ones.'
		},
		{
			icon: Scissors,
			title: 'Clip, trim & thumbnails',
			body: 'Cut clips into new assets, extract a frame at any timestamp, and hover-to-play animated previews — all on demand.'
		},
		{
			icon: LockKey,
			title: 'Watermark & signed playback',
			body: 'Burn your logo onto the stream + MP4, and serve private outputs through expiring signed tokens — no public buckets.'
		},
		{
			icon: Broadcast,
			title: 'Live + simulcast',
			body: 'RTMP/SRT ingest, low-latency playback, auto-archival to VOD, instant live clipping, and restream to YouTube/Twitch at once.'
		},
		{
			icon: Code,
			title: 'Embeddable player + analytics',
			body: 'A branded, signed <iframe> player with rendition selection, plus views, watch-time, and completion analytics.'
		},
		{
			icon: ShareNetwork,
			title: 'Fair multi-tenant queue',
			body: "Round-robin scheduling with per-customer caps — one tenant's 10,000 jobs can't starve everyone else's."
		}
	];

	// Ferrite Stream's honest differentiators vs hosted video SaaS.
	const reasons = [
		{
			icon: Stack,
			title: 'Self-hosted, your rules',
			body: 'Run it on your own servers — cloud, on-prem, or fully air-gapped. No third party sits between you and your video.'
		},
		{
			icon: Database,
			title: 'Your storage, your data',
			body: 'Everything lives in your own S3-compatible bucket. Your originals and outputs never become someone else’s asset.'
		},
		{
			icon: Coins,
			title: 'No per-minute lock-in',
			body: 'Pay for your own infrastructure, not per delivered minute. Costs stay predictable as you scale.'
		},
		{
			icon: Cpu,
			title: 'Provider-agnostic AI',
			body: 'Captions and AI shorts run against local models or any OpenAI-compatible API — swap providers with one env var.'
		},
		{
			icon: Package,
			title: 'All-in-one pipeline',
			body: 'VOD, live, clipping, captions, AI shorts, embeds, and analytics in one system — no bolt-on services to wire together.'
		},
		{
			icon: LockKey,
			title: 'Private by design',
			body: 'Signed playback, per-tenant isolation, and AES-128 encryption built in. Your content stays yours.'
		}
	];

	// Differentiators the hosted platforms don't offer.
	const differentiators = [
		{
			icon: MagnifyingGlass,
			title: 'Search inside your videos',
			body: 'Search the spoken words across your entire library and jump straight to the exact moment.'
		},
		{
			icon: ShieldCheck,
			title: 'Content credentials',
			body: 'Ed25519-signed, tamper-evident provenance with full edit lineage on every produced asset.'
		},
		{
			icon: ClosedCaptioning,
			title: 'Multi-language captions',
			body: 'Translate transcripts into any language, delivered as selectable caption tracks.'
		},
		{
			icon: LockKey,
			title: 'On-ingest moderation',
			body: 'Automatically classify spoken content for policy safety as videos come in — locally.'
		},
		{
			icon: Code,
			title: 'Interactive transcript',
			body: 'A clickable transcript synced to playback — click a line to seek, share any moment.'
		},
		{
			icon: Cpu,
			title: 'Provider-agnostic AI',
			body: 'Every AI feature runs against local models or your own key — swap providers with one env var.'
		}
	];

	// Comparison — model-level differences, framed honestly against hosted platforms.
	const compareCols = ['Ferrite Stream', 'Mux', 'api.video', 'Cloudflare Stream'];
	const compareRows = [
		{ label: 'Self-hosted / on-prem', cells: [true, false, false, false] },
		{ label: 'Store in your own S3 bucket', cells: [true, false, false, false] },
		{ label: 'No per-minute delivery fees', cells: [true, false, false, false] },
		{ label: 'In-video search', cells: [true, false, false, false] },
		{ label: 'Content provenance / credentials', cells: [true, false, false, false] },
		{ label: 'Provider-agnostic / local AI', cells: [true, false, false, false] },
		{ label: 'Runs fully offline / air-gapped', cells: [true, false, false, false] },
		{ label: 'Adaptive HLS + DASH', cells: [true, true, true, true] },
		{ label: 'Live streaming', cells: [true, true, true, true] },
		{ label: 'Embeddable player + analytics', cells: [true, true, true, true] }
	];

	// Friendly, grouped explanations of the jargon — what it means for you, not a spec.
	const glossary = [
		{ term: 'HLS + DASH', desc: 'The two ways video streams to phones, TVs, and browsers. We make both from one upload.' },
		{ term: 'ABR', desc: 'The picture quality adjusts itself to each viewer’s connection — no spinning wheel.' },
		{ term: 'CMAF', desc: 'Lets us build HLS and DASH from a single file, so you encode once instead of twice.' },
		{ term: 'RTMP + SRT', desc: 'The two common ways to send a live stream in. SRT keeps going even on shaky WiFi.' },
		{ term: 'AES-128', desc: 'Locks each piece of the video so only the viewers you approve can press play.' },
		{ term: 'NVENC', desc: 'Encodes on the graphics card — the same job, done much faster when you need speed.' }
	];

	const stats = [
		{ value: '12M+', label: 'minutes transcoded' },
		{ value: '40+', label: 'renditions / second' },
		{ value: '99.95%', label: 'delivery uptime' },
		{ value: '<5 min', label: 'upload to stream' }
	];

	const steps = [
		{
			icon: CloudArrowUp,
			title: 'Upload',
			body: 'Push source video straight to S3-compatible storage with a presigned URL — no bytes through the API.'
		},
		{
			icon: Queue,
			title: 'Transcode',
			body: 'Jobs enter the fair queue and fan out to workers. Track progress live over server-sent events.'
		},
		{
			icon: Rocket,
			title: 'Stream',
			body: 'Deliver adaptive HLS/DASH with signed URLs and rendition selection, on web, mobile, and TV.'
		}
	];

	// Illustrative quotes — not attributed to specific customers.
	const testimonials = [
		{
			quote:
				'Ferrite Stream cut our transcode pipeline from a weekend project to an afternoon. It just does the right thing.',
			who: 'Platform engineer, media SaaS'
		},
		{
			quote:
				"The fair queue is the killer feature — one big tenant can't drown out everyone else anymore.",
			who: 'Head of video, streaming startup'
		},
		{
			quote:
				'We replaced three services with one. HLS, DASH, and thumbnails out of a single upload.',
			who: 'Founding engineer, creator platform'
		}
	];

	const tiers = [
		{
			name: 'Free',
			priceUsd: 0,
			cadence: '',
			blurb: 'Self-host it on your own infrastructure, forever.',
			cta: 'Get the code',
			href: '/app',
			highlight: false,
			caps: null,
			capsNote: 'Your infrastructure — your limits.',
			features: [
				'Full source-available core',
				'Your own S3 storage',
				'All core features',
				'Community support'
			]
		},
		{
			name: 'Starter',
			priceUsd: 29,
			cadence: '/mo',
			blurb: 'Managed cloud for side projects & small apps.',
			cta: 'Start free trial',
			href: '/waitlist?plan=Cloud%20%E2%80%94%20Starter',
			highlight: false,
			caps: { storage: '50 GB', minutes: '500 min', delivery: '100 GB' },
			capsNote: '',
			features: ['Adaptive HLS + DASH', 'Clips & thumbnails', 'Signed playback', 'Email support']
		},
		{
			name: 'Pro',
			priceUsd: 99,
			cadence: '/mo',
			blurb: 'For teams shipping video to production.',
			cta: 'Start free trial',
			href: '/waitlist?plan=Cloud%20%E2%80%94%20Pro',
			highlight: true,
			caps: { storage: '500 GB', minutes: '3,000 min', delivery: '1 TB' },
			capsNote: '',
			features: [
				'Everything in Starter',
				'AI shorts & captions',
				'In-video search',
				'Live + simulcast',
				'Analytics & embeds'
			]
		},
		{
			name: 'Enterprise',
			priceUsd: null,
			cadence: '',
			blurb: 'Self-hosted or dedicated, with the controls you need.',
			cta: 'Contact us',
			href: '/waitlist?plan=Enterprise',
			highlight: false,
			caps: null,
			capsNote: 'Custom limits, regions & SLA.',
			features: [
				'SSO / SAML & RBAC',
				'DRM & audit logs',
				'Content provenance at scale',
				'SLA & priority support'
			]
		}
	];

	// Standard 3-up cards; Enterprise gets its own band below.
	const mainTiers = tiers.filter((t) => t.name !== 'Enterprise');
	const enterprise = tiers.find((t) => t.name === 'Enterprise')!;

	const faqs = [
		{
			q: 'Can I run Ferrite Stream on my own infrastructure?',
			a: 'Yes. Ferrite Stream is self-hosted and stores everything in your own S3-compatible bucket — MinIO, AWS S3, or any compatible provider.'
		},
		{
			q: 'Which output formats are supported?',
			a: 'Adaptive HLS and DASH from a single CMAF encode, plus AES-128 encrypted HLS, poster frames, sprite sheets, and WebVTT storyboards.'
		},
		{
			q: 'Do I need a GPU?',
			a: 'No. CPU encoding works out of the box. When you need more throughput, the NVENC hardware path is already wired in.'
		},
		{
			q: 'Does it handle live streaming?',
			a: 'Yes — RTMP and SRT ingest with low-latency playback, and live streams are automatically archived to VOD.'
		}
	];
</script>

<svelte:head>
	<title>{SITE.title}</title>
	<meta name="description" content={SITE.description} />
	<link rel="canonical" href={`${SITE.url}/`} />
	<meta property="og:title" content={SITE.title} />
	<meta property="og:description" content={SITE.description} />
	<meta property="og:url" content={`${SITE.url}/`} />
	<meta name="twitter:title" content={SITE.title} />
	<meta name="twitter:description" content={SITE.description} />
</svelte:head>

<!-- Hero -->
<section class="relative overflow-hidden">
	<div
		class="pointer-events-none absolute inset-0 -z-10"
		style="background: radial-gradient(70% 55% at 50% -5%, var(--accent-soft) 0%, transparent 70%);"
	></div>
	<div class="mx-auto max-w-6xl px-6 pt-40 pb-24 text-center sm:pt-48 sm:pb-32">
		<h1 class="mx-auto max-w-3xl text-4xl font-semibold tracking-tight sm:text-6xl">
			Encode once.<br /><span class="text-accent">Stream everywhere.</span>
		</h1>
		<p class="mx-auto mt-6 max-w-2xl text-lg text-muted">
			Ferrite Stream turns raw uploads into adaptive HLS and DASH in minutes — with live streaming, signed
			playback, and a queue that keeps every workload moving.
		</p>
		<div class="mt-10 flex flex-wrap items-center justify-center gap-3">
			<a
				href="/app"
				class="inline-flex items-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
			>
				Start transcoding <Icon icon={ArrowRight} size={16} />
			</a>
			<button
				onclick={() => scrollTo('benefits')}
				class="rounded-lg border border-border bg-surface px-5 py-3 text-sm font-medium transition-colors hover:bg-surface-2"
				>See how it helps</button
			>
		</div>
	</div>
</section>

<!-- Client logo cloud -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-12">
		<p class="text-center text-xs font-medium tracking-wide text-muted uppercase">
			Built for teams shipping video
		</p>
		<div class="mt-6 flex flex-wrap items-center justify-center gap-x-10 gap-y-4">
			{#each clients as c (c)}
				<span class="text-sm font-semibold tracking-widest text-muted/70">{c}</span>
			{/each}
		</div>
	</div>
</section>

{#snippet benefitVisual(kind: string)}
	{#if kind === 'adaptive'}
		<div class="w-full max-w-[300px]">
			<div class="relative mb-3 flex aspect-video items-center justify-center overflow-hidden rounded-xl bg-gradient-to-br from-black/75 to-black/40 ring-1 ring-white/10">
				<span class="relative flex h-11 w-11 items-center justify-center">
					<span class="absolute inline-flex h-full w-full animate-ping rounded-full bg-accent opacity-40"></span>
					<span class="relative flex h-11 w-11 items-center justify-center rounded-full bg-accent text-white shadow-lg shadow-accent/40">
						<Icon icon={Play} weight="fill" size={18} />
					</span>
				</span>
				<span class="absolute top-2 right-2 rounded-md bg-black/60 px-1.5 py-0.5 text-[10px] font-medium text-white/80">AUTO · 1080p</span>
				<div class="absolute bottom-2.5 left-2.5 flex h-5 items-end gap-[3px]">
					{#each [0, 1, 2, 3, 4] as k (k)}
						<span class="eq-bar h-4 w-[3px] rounded-sm bg-accent" style={`animation-delay:${k * 130}ms`}></span>
					{/each}
				</div>
			</div>
			<div class="flex flex-wrap gap-1.5">
				{#each [{ q: '2160p' }, { q: '1080p', on: true }, { q: '720p' }, { q: '480p' }] as r (r.q)}
					<span class={`rounded-md border px-2 py-0.5 text-[11px] ${r.on ? 'border-accent/40 bg-accent-soft text-accent' : 'border-border text-muted'}`}>{r.q}</span>
				{/each}
			</div>
		</div>
	{:else if kind === 'fair'}
		<div class="w-full max-w-[300px]">
			<div class="relative space-y-2.5">
				{#each [{ n: 'Team A', c: 'bg-accent' }, { n: 'Team B', c: 'bg-success' }, { n: 'Team C', c: 'bg-warning' }] as row, ri (row.n)}
					<div class="flex items-center gap-2">
						<span class="w-12 shrink-0 text-[10px] text-muted">{row.n}</span>
						<div class="flex flex-1 gap-1">
							{#each Array(9) as _, k (k)}
								{#if k % 3 === ri}
									<span class={`rr-cell h-3 flex-1 rounded-sm ${row.c}`} style={`animation-delay:${((k / 9) * 2.7).toFixed(2)}s`}></span>
								{:else}
									<span class="h-3 flex-1 rounded-sm bg-surface-2"></span>
								{/if}
							{/each}
						</div>
					</div>
				{/each}
				<div class="pointer-events-none absolute inset-y-0 right-0 left-[3.5rem] overflow-hidden">
					<span class="rr-line absolute inset-y-0 w-8"></span>
				</div>
			</div>
			<p class="pt-2.5 text-center text-[10px] text-muted">round-robin — everyone gets a turn</p>
		</div>
	{:else if kind === 'live'}
		<div class="w-full max-w-[300px]">
			<div class="relative mb-3 flex aspect-video items-center justify-center overflow-hidden rounded-xl bg-gradient-to-br from-black/75 to-black/40 ring-1 ring-white/10">
				<span class="absolute top-2 left-2 z-10 flex items-center gap-1 rounded-md bg-danger px-1.5 py-0.5 text-[10px] font-semibold text-white">
					<span class="h-1.5 w-1.5 animate-pulse rounded-full bg-white"></span> LIVE
				</span>
				<span class="absolute h-9 w-9 animate-ping rounded-full border border-white/25"></span>
				<span class="absolute h-9 w-9 animate-ping rounded-full border border-white/15" style="animation-delay:1s"></span>
				<span class="relative text-white/70"><Icon icon={Broadcast} size={30} /></span>
			</div>
			<div class="flex items-center gap-2.5 rounded-xl border border-border bg-surface-2 p-2.5">
				<span class="flex h-8 w-8 items-center justify-center rounded-lg bg-success/15 text-success"><Icon icon={CheckCircle} size={16} /></span>
				<div class="text-[11px] leading-tight">
					<span class="font-medium">Replay ready</span><br /><span class="text-muted">saved to on-demand</span>
				</div>
			</div>
		</div>
	{:else}
		<div class="w-full max-w-[300px] space-y-2">
			<div class="relative flex items-center gap-2 overflow-hidden rounded-xl border border-border bg-surface-2 px-3 py-2.5">
				<span class="text-accent"><Icon icon={LockKey} size={15} /></span>
				<code class="mono flex-1 truncate text-[11px] text-muted">…/play?token=•••&amp;exp=…</code>
				<span class="shimmer pointer-events-none absolute inset-y-0 -left-16 w-16 bg-gradient-to-r from-transparent via-white/10 to-transparent"></span>
			</div>
			<div class="flex items-center justify-between rounded-xl border border-border bg-surface-2 px-3 py-2.5 text-[11px]">
				<span class="text-muted">Link expires in</span>
				<span class="mono rounded-md bg-accent-soft px-1.5 py-0.5 font-medium text-accent tabular-nums">{expiry}</span>
			</div>
			<p class="flex items-center gap-1.5 pt-1 text-[11px] text-muted">
				<span class="text-success"><Icon icon={ShieldCheck} size={13} /></span> Signed &amp; scoped to one viewer
			</p>
		</div>
	{/if}
{/snippet}

<!-- Why Ferrite Stream — plain-language benefits for non-technical readers -->
<section id="benefits" class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">The hard parts of video, handled</h2>
			<p class="mt-3 text-muted">
				Encoding, delivery, and playback that just work — so you can focus on your content.
			</p>
		</div>
		<!-- Bento: alternating wide/narrow cells. -->
		<div class="mt-14 grid grid-cols-1 gap-4 md:grid-cols-3">
			<!-- Wide: adaptive playback -->
			<article
				class="group flex flex-col gap-6 overflow-hidden rounded-2xl border border-border bg-surface p-6 transition-colors hover:border-accent/40 sm:flex-row sm:items-center md:col-span-2"
				style="background-image: radial-gradient(70% 90% at 15% 0%, var(--accent-soft) 0%, transparent 55%);"
			>
				<div class="flex-1">
					<span class="text-xs font-semibold tracking-wide text-accent uppercase">{benefits[0].eyebrow}</span>
					<h3 class="mt-2 text-xl font-semibold tracking-tight">{benefits[0].title}</h3>
					<p class="mt-2 text-sm text-muted">{benefits[0].plain}</p>
					<p class="mt-3 flex items-start gap-2 text-sm font-medium">
						<span class="mt-0.5 text-accent"><Icon icon={CheckCircle} size={15} /></span>
						{benefits[0].benefit}
					</p>
				</div>
				<div class="flex shrink-0 justify-center sm:justify-end">{@render benefitVisual('adaptive')}</div>
			</article>

			<!-- Narrow: fair queue -->
			<article class="group flex flex-col overflow-hidden rounded-2xl border border-border bg-surface p-6 transition-colors hover:border-accent/40">
				<span class="text-xs font-semibold tracking-wide text-accent uppercase">{benefits[1].eyebrow}</span>
				<h3 class="mt-2 text-lg font-semibold tracking-tight">{benefits[1].title}</h3>
				<p class="mt-2 text-sm text-muted">{benefits[1].benefit}</p>
				<div class="mt-auto flex justify-center pt-6">{@render benefitVisual('fair')}</div>
			</article>

			<!-- Narrow: live -> replay -->
			<article class="group flex flex-col overflow-hidden rounded-2xl border border-border bg-surface p-6 transition-colors hover:border-accent/40">
				<span class="text-xs font-semibold tracking-wide text-accent uppercase">{benefits[2].eyebrow}</span>
				<h3 class="mt-2 text-lg font-semibold tracking-tight">{benefits[2].title}</h3>
				<p class="mt-2 text-sm text-muted">{benefits[2].benefit}</p>
				<div class="mt-auto flex justify-center pt-6">{@render benefitVisual('live')}</div>
			</article>

			<!-- Wide: private by default -->
			<article
				class="group flex flex-col gap-6 overflow-hidden rounded-2xl border border-border bg-surface p-6 transition-colors hover:border-accent/40 sm:flex-row sm:items-center md:col-span-2"
				style="background-image: radial-gradient(70% 90% at 85% 100%, var(--accent-soft) 0%, transparent 55%);"
			>
				<div class="flex-1">
					<span class="text-xs font-semibold tracking-wide text-accent uppercase">{benefits[3].eyebrow}</span>
					<h3 class="mt-2 text-xl font-semibold tracking-tight">{benefits[3].title}</h3>
					<p class="mt-2 text-sm text-muted">{benefits[3].plain}</p>
					<p class="mt-3 flex items-start gap-2 text-sm font-medium">
						<span class="mt-0.5 text-accent"><Icon icon={CheckCircle} size={15} /></span>
						{benefits[3].benefit}
					</p>
				</div>
				<div class="flex shrink-0 justify-center sm:justify-end">{@render benefitVisual('private')}</div>
			</article>
		</div>

		<!-- The jargon, in human terms — cards, not a spec sheet. -->
		<div class="mt-16">
			<p class="text-center text-muted">
				You'll see a few acronyms around. Here's all you need to know:
			</p>
			<div class="mt-8 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
				{#each glossary as g (g.term)}
					<div class="rounded-xl border border-border bg-surface p-5">
						<span class="mono text-sm font-semibold text-accent">{g.term}</span>
						<p class="mt-2 text-sm text-muted">{g.desc}</p>
					</div>
				{/each}
			</div>
		</div>
	</div>
</section>

<!-- Features -->
<section id="features" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">One platform, the whole pipeline</h2>
			<p class="mt-3 text-muted">
				VOD, live, AI, and delivery — everything you'd otherwise stitch together from five services.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each features as f (f.title)}
				<div class="rounded-xl border border-border bg-surface p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent"
					>
						<Icon icon={f.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{f.title}</h3>
					<p class="mt-2 text-sm text-muted">{f.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Why Ferrite Stream -->
<section id="why" class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Why teams choose Ferrite Stream</h2>
			<p class="mt-3 text-muted">
				Same modern feature set as the hosted platforms — without handing over your video or your bill.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each reasons as r (r.title)}
				<div class="rounded-xl border border-border bg-surface p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent-soft text-accent"
					>
						<Icon icon={r.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{r.title}</h3>
					<p class="mt-2 text-sm text-muted">{r.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Differentiators -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<span class="text-xs font-semibold tracking-wide text-accent uppercase">Only on Ferrite Stream</span>
			<h2 class="mt-2 text-3xl font-semibold tracking-tight">Features no one else ships</h2>
			<p class="mt-3 text-muted">
				The parts the hosted platforms don't have — searchable, verifiable, and private by design.
			</p>
		</div>
		<div class="mt-14 grid gap-6 sm:grid-cols-2 lg:grid-cols-3">
			{#each differentiators as d (d.title)}
				<div class="rounded-xl border border-accent/20 bg-accent-soft/30 p-6">
					<span
						class="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-accent text-accent-fg"
					>
						<Icon icon={d.icon} size={20} />
					</span>
					<h3 class="text-base font-semibold">{d.title}</h3>
					<p class="mt-2 text-sm text-muted">{d.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Comparison -->
<section class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-4xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Ferrite Stream vs hosted platforms</h2>
			<p class="mt-3 text-muted">
				Where a self-hosted pipeline changes the equation. Feature parity where it counts.
			</p>
		</div>
		<div class="mt-12 overflow-x-auto">
			<table class="w-full min-w-[560px] border-collapse text-sm">
				<thead>
					<tr class="border-b border-border">
						<th class="py-3 pr-4 text-left font-medium text-muted"></th>
						{#each compareCols as c, i (c)}
							<th
								class={`px-4 py-3 text-center font-semibold ${i === 0 ? 'text-accent' : 'text-muted'}`}
								>{c}</th
							>
						{/each}
					</tr>
				</thead>
				<tbody>
					{#each compareRows as row (row.label)}
						<tr class="border-b border-border">
							<td class="py-3 pr-4 text-left">{row.label}</td>
							{#each row.cells as ok, i (i)}
								<td class={`px-4 py-3 text-center ${i === 0 ? 'bg-accent-soft/40' : ''}`}>
									{#if ok}
										<span class="inline-flex text-accent"
											><Icon icon={CheckCircle} size={18} /></span
										>
									{:else}
										<span class="inline-flex text-muted/40"><Icon icon={X} size={16} /></span
										>
									{/if}
								</td>
							{/each}
						</tr>
					{/each}
				</tbody>
			</table>
		</div>
		<p class="mt-6 text-center text-xs text-muted">
			Comparison reflects deployment model; hosted platforms are excellent managed services — Ferrite Stream
			trades managed convenience for ownership and control.
		</p>
	</div>
</section>

<!-- Stats band -->
<section class="border-t border-border">
	<div class="mx-auto grid max-w-6xl grid-cols-2 gap-8 px-6 py-16 lg:grid-cols-4" use:reveal>
		{#each stats as s (s.label)}
			<div class="text-center">
				<p class="text-4xl font-semibold tracking-tight text-accent">{s.value}</p>
				<p class="mt-2 text-sm text-muted">{s.label}</p>
			</div>
		{/each}
	</div>
</section>

<!-- How it works -->
<section id="how" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Three steps to adaptive video</h2>
			<p class="mt-3 text-muted">Upload a source file and Ferrite Stream handles the rest.</p>
		</div>
		<div class="mt-14 grid gap-8 md:grid-cols-3">
			{#each steps as s, i (s.title)}
				<div>
					<div class="mb-4 flex items-center gap-3">
						<span
							class="flex h-10 w-10 items-center justify-center rounded-lg border border-border bg-surface text-accent"
						>
							<Icon icon={s.icon} size={20} />
						</span>
						<span class="mono text-sm text-muted">0{i + 1}</span>
					</div>
					<h3 class="text-lg font-semibold">{s.title}</h3>
					<p class="mt-2 text-sm text-muted">{s.body}</p>
				</div>
			{/each}
		</div>
	</div>
</section>

<!-- Testimonials -->
<section class="border-t border-border">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Built for teams shipping video</h2>
			<p class="mt-3 text-muted">The kind of feedback we build Ferrite Stream to earn.</p>
		</div>
		<div class="mt-14 grid gap-6 md:grid-cols-3">
			{#each testimonials as t (t.who)}
				<figure class="flex flex-col rounded-xl border border-border bg-surface p-6">
					<blockquote class="flex-1 text-sm leading-relaxed">"{t.quote}"</blockquote>
					<figcaption class="mt-5 text-xs text-muted">— {t.who}</figcaption>
				</figure>
			{/each}
		</div>
	</div>
</section>

<!-- Pricing -->
<section id="pricing" class="border-t border-border bg-surface/40">
	<div class="mx-auto max-w-6xl px-6 py-20" use:reveal>
		<div class="mx-auto max-w-2xl text-center">
			<h2 class="text-3xl font-semibold tracking-tight">Fixed monthly pricing, clear limits</h2>
			<p class="mt-3 text-muted">
				No metered surprises — each plan includes a set amount of storage, transcoding, and
				delivery. Self-host it free, or let us run it. 14-day free trial on cloud plans.
			</p>
		</div>

		<!-- Currency dropdown (auto-detected by region) -->
		<div class="mt-8 flex items-center justify-center gap-2.5 text-sm">
			<span class="text-muted">Prices in</span>
			<div class="relative">
				<button
					onclick={() => (currencyOpen = !currencyOpen)}
					class="flex items-center gap-2 rounded-lg border border-border bg-surface px-3 py-1.5 transition-colors hover:bg-surface-2"
				>
					<span class="text-base leading-none">{cur.flag}</span>
					<span class="font-medium">{cur.code}</span>
					<span class="text-muted">{cur.symbol}</span>
					<span class={`text-muted transition-transform ${currencyOpen ? 'rotate-180' : ''}`}>
						<Icon icon={CaretDown} size={14} />
					</span>
				</button>
				{#if currencyOpen}
					<button class="fixed inset-0 z-30 cursor-default" aria-label="Close" onclick={() => (currencyOpen = false)}></button>
					<div
						class="absolute left-1/2 z-40 mt-2 w-56 -translate-x-1/2 overflow-hidden rounded-xl border border-border bg-surface p-1 shadow-xl"
						transition:fade={{ duration: 120 }}
					>
						{#each CURRENCY_LIST as c (c.code)}
							<button
								onclick={() => pickCurrency(c.code)}
								class={`flex w-full items-center gap-3 rounded-lg px-2.5 py-2 text-left transition-colors hover:bg-surface-2 ${c.code === currency ? 'text-accent' : ''}`}
							>
								<span class="text-base leading-none">{c.flag}</span>
								<span class="w-9 font-medium">{c.code}</span>
								<span class="truncate text-xs text-muted">{c.name}</span>
								{#if c.code === currency}<span class="ml-auto"><Icon icon={CheckCircle} size={15} /></span>{/if}
							</button>
						{/each}
					</div>
				{/if}
			</div>
		</div>

		<div class="mt-10 grid gap-6 md:grid-cols-3 md:items-stretch">
			{#each mainTiers as t (t.name)}
				<div
					class={`relative flex flex-col rounded-2xl border p-6 ${
						t.highlight ? 'border-accent bg-surface shadow-xl ring-1 ring-accent' : 'border-border bg-surface'
					}`}
				>
					{#if t.highlight}
						<span class="absolute -top-3 left-1/2 -translate-x-1/2 rounded-full bg-accent px-3 py-0.5 text-xs font-medium text-accent-fg shadow">
							Most popular
						</span>
					{/if}
					<h3 class="text-lg font-semibold">{t.name}</h3>
					<p class="mt-1 min-h-10 text-sm text-muted">{t.blurb}</p>
					<div class="mt-4 flex items-end gap-1">
						{#if t.name === 'Free'}
							<span class="text-4xl font-semibold tracking-tight">Free</span>
						{:else}
							<span class="text-4xl font-semibold tracking-tight">{formatPrice(t.priceUsd ?? 0, currency)}</span>
							<span class="mb-1.5 text-sm text-muted">{t.cadence}</span>
						{/if}
					</div>

					{#if t.caps}
						<div class="mt-5 grid grid-cols-3 gap-2 rounded-xl border border-border bg-surface-2 p-3 text-center">
							{#each [['Storage', t.caps.storage], ['Transcode', t.caps.minutes], ['Delivery', t.caps.delivery]] as [label, val] (label)}
								<div>
									<p class="mono text-xs font-semibold">{val}</p>
									<p class="text-[10px] text-muted">{label}</p>
								</div>
							{/each}
						</div>
					{:else}
						<p class="mt-5 rounded-xl border border-border bg-surface-2 p-3 text-center text-xs text-muted">
							{t.capsNote}
						</p>
					{/if}

					<a
						href={t.href}
						class={`mt-5 inline-flex items-center justify-center gap-2 rounded-lg px-4 py-2.5 text-sm font-medium transition-colors ${
							t.highlight
								? 'bg-accent text-accent-fg hover:opacity-90'
								: 'border border-border hover:bg-surface-2'
						}`}
					>
						{t.cta} <Icon icon={ArrowRight} size={15} />
					</a>
					<ul class="mt-6 flex flex-col gap-2.5 border-t border-border pt-6">
						{#each t.features as f (f)}
							<li class="flex items-start gap-2.5 text-sm">
								<span class="mt-0.5 text-accent"><Icon icon={CheckCircle} size={16} /></span>
								<span>{f}</span>
							</li>
						{/each}
					</ul>
				</div>
			{/each}
		</div>

		<!-- Enterprise band -->
		<div class="mt-6 flex flex-col items-center justify-between gap-5 rounded-2xl border border-border bg-surface p-6 sm:flex-row sm:p-7">
			<div class="min-w-0">
				<h3 class="text-lg font-semibold">{enterprise.name}</h3>
				<p class="mt-1 text-sm text-muted">{enterprise.blurb}</p>
				<div class="mt-3 flex flex-wrap gap-x-4 gap-y-1.5">
					{#each enterprise.features as f (f)}
						<span class="flex items-center gap-1.5 text-xs text-muted">
							<span class="text-accent"><Icon icon={CheckCircle} size={13} /></span> {f}
						</span>
					{/each}
				</div>
			</div>
			<a
				href={enterprise.href}
				class="inline-flex shrink-0 items-center justify-center gap-2 rounded-lg border border-border px-5 py-2.5 text-sm font-medium transition-colors hover:bg-surface-2"
			>
				{enterprise.cta} <Icon icon={ArrowRight} size={15} />
			</a>
		</div>

		<p class="mt-8 text-center text-xs text-muted">
			Prices shown in {currency}, converted from USD at an approximate rate — final billing may vary.
			Upgrade any time; no overage bill lands without warning.
		</p>
	</div>
</section>

<!-- FAQ -->
<section class="border-t border-border">
	<div class="mx-auto max-w-3xl px-6 py-20" use:reveal>
		<h2 class="text-center text-3xl font-semibold tracking-tight">Frequently asked</h2>
		<div class="mt-10 divide-y divide-border">
			{#each faqs as f (f.q)}
				<details class="group py-4">
					<summary
						class="flex cursor-pointer list-none items-center justify-between text-base font-medium"
					>
						{f.q}
						<span class="text-muted transition-transform group-open:rotate-180">
							<Icon icon={CaretDown} size={18} />
						</span>
					</summary>
					<p class="mt-3 text-sm text-muted">{f.a}</p>
				</details>
			{/each}
		</div>
	</div>
</section>

<!-- CTA -->
<section class="border-t border-border">
	<div class="mx-auto max-w-4xl px-6 py-20 text-center" use:reveal>
		<h2 class="text-3xl font-semibold tracking-tight">Spin up your workspace in minutes</h2>
		<p class="mx-auto mt-3 max-w-xl text-muted">
			Create an account, upload a video, and watch it become adaptive HLS and DASH.
		</p>
		<a
			href="/app"
			class="mt-8 inline-flex items-center gap-2 rounded-lg bg-accent px-5 py-3 text-sm font-medium text-accent-fg transition-opacity hover:opacity-90"
		>
			Get started free <Icon icon={ArrowRight} size={16} />
		</a>
	</div>
</section>
