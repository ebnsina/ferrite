// Canonical site metadata, shared across <svelte:head> blocks. Update `url` to
// your production origin if it isn't ferrite.io.
export const SITE = {
	name: 'Ferrite Stream',
	url: 'https://ferrite.io',
	title: 'Ferrite Stream — self-hosted adaptive video (VOD + live)',
	description:
		'Ferrite Stream turns raw uploads into adaptive HLS & DASH — with live streaming, AI shorts & captions, a fair queue, and signed playback. Self-hosted on your own S3 storage.',
	ogImage: 'https://ferrite.io/og.png'
} as const;
