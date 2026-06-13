import { describe, it, expect } from 'vitest';
import { failFrom } from './form-errors';
import { ApiError } from './api';

// fail(status, data) from @sveltejs/kit returns an ActionFailure whose shape is
// { status, data }. We assert against that directly.

describe('failFrom', () => {
	it('maps an ApiError to fail(status) with code/message and the action tag', () => {
		const result = failFrom('create', new ApiError('slug_taken', 'slug already in use', 409));

		expect(result).toMatchObject({
			status: 409,
			data: {
				action: 'create',
				code: 'slug_taken',
				message: 'slug already in use'
			}
		});
	});

	it('maps an unknown error to a generic 500', () => {
		const result = failFrom('edit', new Error('boom'));

		expect(result).toMatchObject({
			status: 500,
			data: {
				action: 'edit',
				code: 'internal_error',
				message: 'An unexpected error occurred'
			}
		});
	});

	it('passes extras through on the ApiError branch', () => {
		const result = failFrom('delete', new ApiError('forbidden', 'no', 403), { id: 'm1' });

		expect(result).toMatchObject({
			status: 403,
			data: { action: 'delete', code: 'forbidden', message: 'no', id: 'm1' }
		});
	});

	it('passes extras through on the unknown-error branch', () => {
		const result = failFrom('removeMember', 'not-an-error', { memberId: 'x9' });

		expect(result).toMatchObject({
			status: 500,
			data: {
				action: 'removeMember',
				code: 'internal_error',
				message: 'An unexpected error occurred',
				memberId: 'x9'
			}
		});
	});
});
