// Turn technical error strings (worker/API internals, env-var names, ffmpeg
// output) into plain, friendly messages for people. Anything unrecognised
// falls back to a calm generic line rather than leaking a raw stack.

interface Rule {
	match: RegExp;
	message: string;
}

// Order matters — first match wins. Keep messages plain and blame-free.
const RULES: Rule[] = [
	{
		match: /transcriber|whisper|no transcriber|FERRITE_(WHISPER|AI)/i,
		message: 'Automatic captions aren’t set up yet, so this video couldn’t be transcribed.'
	},
	{
		match: /translation is not configured|translate/i,
		message: 'Automatic translation isn’t set up yet.'
	},
	{
		match: /timed out|timeout|stale/i,
		message: 'This took too long to process and was stopped. Please try again.'
	},
	{
		match: /clip/i,
		message: 'We couldn’t create this clip. Please try again.'
	},
	{
		match: /storage|s3|bucket|upload/i,
		message: 'There was a problem saving the result. Please try again.'
	},
	{
		match: /ffmpeg|ffprobe|transcode|encode|probe|packaging|codec|unsupported/i,
		message: 'We couldn’t process this video — the file may be unsupported or damaged.'
	},
	{
		match: /db error|database|sqlx|connection|pool/i,
		message: 'Something went wrong on our end. Please try again in a moment.'
	}
];

const GENERIC = 'Something went wrong while processing this video.';

/** Friendly version of a job's failure message. */
export function humanizeJobError(raw?: string | null): string {
	if (!raw) return GENERIC;
	for (const rule of RULES) {
		if (rule.match.test(raw)) return rule.message;
	}
	// Unknown message: strip any env-var / path noise and reuse if it reads
	// like a sentence, otherwise fall back.
	const cleaned = raw.replace(/FERRITE_[A-Z_*]+/g, '').replace(/\s{2,}/g, ' ').trim();
	return cleaned.length > 0 && cleaned.length < 120 && !/[/{}]|error:/i.test(cleaned)
		? cleaned
		: GENERIC;
}

/** Friendly version of an arbitrary API/error message for toasts and inline use. */
export function humanizeError(raw?: string | null, fallback = 'Something went wrong. Please try again.'): string {
	if (!raw) return fallback;
	for (const rule of RULES) {
		if (rule.match.test(raw)) return rule.message;
	}
	const cleaned = raw.replace(/FERRITE_[A-Z_*]+/g, '').replace(/\s{2,}/g, ' ').trim();
	// Keep short, human-sounding server messages (e.g. "email already registered");
	// drop anything that looks like an internal dump.
	return cleaned.length > 0 && cleaned.length < 140 && !/[/{}]|::|panicked|unwrap/i.test(cleaned)
		? cleaned
		: fallback;
}
