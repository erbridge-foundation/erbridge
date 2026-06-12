import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { render, screen, fireEvent, cleanup } from '@testing-library/svelte';
import AuditDetailsDialog from './AuditDetailsDialog.svelte';

// jsdom does not implement the native <dialog> modal API; stub it so the
// component's open/close $effect runs without throwing. The stub flips `.open`
// the way the real API does, which is all the component reads.
beforeEach(() => {
	HTMLDialogElement.prototype.showModal = function (this: HTMLDialogElement) {
		this.open = true;
	};
	HTMLDialogElement.prototype.close = function (this: HTMLDialogElement) {
		this.open = false;
		this.dispatchEvent(new Event('close'));
	};
});

afterEach(() => {
	cleanup();
});

describe('AuditDetailsDialog', () => {
	it('renders each details field as a key/value pair', () => {
		render(AuditDetailsDialog, {
			props: {
				open: true,
				onClose: () => {},
				details: { member_name: 'Wasp 222', member_type: 'character', permission: 'admin' }
			}
		});

		// The snapshotted member name is shown verbatim — answering "who was added".
		expect(screen.getByText('member_name')).toBeInTheDocument();
		expect(screen.getByText('Wasp 222')).toBeInTheDocument();
		expect(screen.getByText('member_type')).toBeInTheDocument();
		expect(screen.getByText('permission')).toBeInTheDocument();
		expect(screen.getByText('admin')).toBeInTheDocument();
	});

	it('formats non-string scalar values', () => {
		render(AuditDetailsDialog, {
			props: {
				open: true,
				onClose: () => {},
				details: { eve_entity_id: 95465499 }
			}
		});

		expect(screen.getByText('eve_entity_id')).toBeInTheDocument();
		expect(screen.getByText('95465499')).toBeInTheDocument();
	});

	it('shows the empty state for an empty details object', () => {
		render(AuditDetailsDialog, {
			props: { open: true, onClose: () => {}, details: {} }
		});

		expect(screen.getByRole('status')).toBeInTheDocument();
		// No key/value list rendered.
		expect(document.querySelector('dl.kv')).toBeNull();
	});

	it('shows the empty state for a non-object payload', () => {
		render(AuditDetailsDialog, {
			props: { open: true, onClose: () => {}, details: null }
		});

		expect(screen.getByRole('status')).toBeInTheDocument();
	});

	it('calls onClose when the close button is clicked (no other side effects)', async () => {
		const onClose = vi.fn();
		render(AuditDetailsDialog, {
			props: {
				open: true,
				onClose,
				details: { member_name: 'Wasp 222' }
			}
		});

		await fireEvent.click(screen.getByRole('button'));
		expect(onClose).toHaveBeenCalledTimes(1);
		// The rendered content is unchanged — dismissing mutates nothing.
		expect(screen.getByText('Wasp 222')).toBeInTheDocument();
	});
});
