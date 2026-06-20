import { describe, it, expect, vi, afterEach } from 'vitest';
import { render, screen, cleanup, fireEvent } from '@testing-library/svelte';
import SigEditDialog from './SigEditDialog.svelte';
import type { ScanResult } from '$lib/map/types';

// globals: false in vitest.config → register cleanup explicitly.
afterEach(cleanup);

function scan(overrides: Partial<ScanResult> = {}): ScanResult {
	return {
		sig_id: 'ABC-123',
		group: 'Cosmic Signature',
		site_type: 'Data Site',
		name: 'Unsecured Perimeter Transponder Farm',
		wh_type: null,
		created_at: '2026-06-18T00:00:00.000Z',
		created_by: 0,
		updated_at: '2026-06-18T00:00:00.000Z',
		updated_by: 0,
		...overrides
	};
}

describe('SigEditDialog', () => {
	it('renders the add form (sig id + type + name) when open in add mode', () => {
		render(SigEditDialog, {
			props: { open: true, mode: 'add', existingIds: [], onSave: () => {} }
		});
		expect(screen.getByRole('dialog', { name: 'Add signature' })).toBeInTheDocument();
		expect(screen.getByLabelText('Signature ID')).toBeInTheDocument();
		expect(screen.getByLabelText('Type')).toBeInTheDocument();
		expect(screen.getByLabelText('Name')).toBeInTheDocument();
	});

	it('seeds the fields from the scan in edit mode', () => {
		render(SigEditDialog, {
			props: { open: true, mode: 'edit', scan: scan(), existingIds: [], onSave: () => {} }
		});
		expect(screen.getByRole('dialog', { name: 'Edit signature' })).toBeInTheDocument();
		expect(screen.getByLabelText<HTMLInputElement>('Signature ID').value).toBe('ABC-123');
		expect(screen.getByLabelText<HTMLInputElement>('Name').value).toBe(
			'Unsecured Perimeter Transponder Farm'
		);
	});

	it('shows the gated notice (no fields) for a wormhole sig', () => {
		render(SigEditDialog, {
			props: {
				open: true,
				mode: 'edit',
				scan: scan({ site_type: 'Wormhole', name: null, wh_type: 'K162' }),
				existingIds: [],
				onSave: () => {}
			}
		});
		expect(screen.getByText(/Editing wormhole signatures isn't available yet/)).toBeInTheDocument();
		expect(screen.queryByLabelText('Signature ID')).toBeNull();
		expect(screen.getByRole('button', { name: 'Close' })).toBeInTheDocument();
	});

	it('saves a valid new signature', async () => {
		const onSave = vi.fn();
		render(SigEditDialog, {
			props: { open: true, mode: 'add', existingIds: ['XYZ-999'], onSave }
		});
		await fireEvent.input(screen.getByLabelText('Signature ID'), { target: { value: 'NEW-001' } });
		await fireEvent.input(screen.getByLabelText('Name'), { target: { value: 'Some Site' } });
		await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
		expect(onSave).toHaveBeenCalledWith(
			expect.objectContaining({ sig_id: 'NEW-001', name: 'Some Site' })
		);
	});

	it('blocks a duplicate sig id and shows an error', async () => {
		const onSave = vi.fn();
		render(SigEditDialog, {
			props: { open: true, mode: 'add', existingIds: ['ABC-123'], onSave }
		});
		await fireEvent.input(screen.getByLabelText('Signature ID'), { target: { value: 'abc-123' } });
		await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
		expect(onSave).not.toHaveBeenCalled();
		expect(screen.getByRole('alert')).toHaveTextContent(/already exists/);
	});

	it('blocks an empty sig id', async () => {
		const onSave = vi.fn();
		render(SigEditDialog, {
			props: { open: true, mode: 'add', existingIds: [], onSave }
		});
		await fireEvent.click(screen.getByRole('button', { name: 'OK' }));
		expect(onSave).not.toHaveBeenCalled();
		expect(screen.getByRole('alert')).toHaveTextContent(/required/);
	});
});
