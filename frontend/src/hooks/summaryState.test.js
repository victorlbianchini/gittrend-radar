import test from 'node:test';
import assert from 'node:assert/strict';
import { mergeSummaryRecords, summaryProgress } from './summaryState.js';

test('page changes discard stale summaries and ignore late old-page records', () => {
  const current = { old: { key: 'old', status: 'ready', summary: 'Old page' }, kept: { key: 'kept', status: 'running' } };
  const merged = mergeSummaryRecords(current, [{ key: 'old', status: 'ready' }, { key: 'new', status: 'ready' }], ['kept', 'new']);
  assert.deepEqual(Object.keys(merged).sort(), ['kept', 'new']);
  assert.equal(merged.kept.status, 'running');
  assert.equal(current.old.summary, 'Old page');
});

test('progress uses persisted keys, live results, missing and failed summaries', () => {
  const items = [{ key: 'a', summary: { key: 'hash-a', status: 'pending' } }, { key: 'b', summary: { key: 'hash-b', status: 'error' } }, { key: 'c' }, { key: 'd', summary: { status: 'running' } }];
  assert.deepEqual(summaryProgress(items, { 'hash-a': { status: 'ready' } }), { ready: 1, running: 1, pending: 1, error: 1, total: 4 });
});

test('unknown backend status remains pending rather than counting as completed', () => {
  assert.deepEqual(summaryProgress([{ key: 'a', summary: { status: 'future-status' } }], {}), { ready: 0, running: 0, pending: 1, error: 0, total: 1 });
});
