// Client-side form validation schemas (zod). Mirror the API's own rules so users
// get instant feedback; the server still validates authoritatively.
import { z } from 'zod';

export const loginSchema = z.object({
	email: z.string().trim().min(1, 'Email is required').email('Enter a valid email'),
	password: z.string().min(1, 'Password is required')
});

export const signupSchema = z.object({
	workspace: z.string().trim().min(1, 'Workspace name is required').max(100),
	email: z.string().trim().min(1, 'Email is required').email('Enter a valid email'),
	password: z.string().min(8, 'Password must be at least 8 characters')
});

export const profileNameSchema = z.object({
	name: z.string().trim().min(1, 'Name is required').max(80, 'Name is too long')
});

export const passwordSchema = z
	.object({
		current_password: z.string().min(1, 'Enter your current password'),
		new_password: z.string().min(8, 'Password must be at least 8 characters'),
		confirm: z.string().min(1, 'Confirm your new password')
	})
	.refine((v) => v.new_password === v.confirm, {
		message: 'Passwords do not match',
		path: ['confirm']
	});

export const inviteSchema = z.object({
	email: z.string().trim().min(1, 'Email is required').email('Enter a valid email'),
	role: z.enum(['member', 'admin'])
});

export const apiKeySchema = z.object({
	name: z.string().trim().min(1, 'Give the key a name').max(100)
});

export const liveStreamSchema = z.object({
	name: z.string().trim().min(1, 'Stream name is required').max(100)
});

/** Validate `data` against `schema`; return field errors keyed by field name. */
export function validate<T>(
	schema: z.ZodType<T>,
	data: unknown
): { ok: true; data: T } | { ok: false; errors: Record<string, string> } {
	const result = schema.safeParse(data);
	if (result.success) return { ok: true, data: result.data };
	const errors: Record<string, string> = {};
	for (const issue of result.error.issues) {
		const key = String(issue.path[0] ?? '_');
		errors[key] ??= issue.message;
	}
	return { ok: false, errors };
}
