export const summaryKey = (item) => item.summary?.key || item.key;

// Reject records from a previous catalog page; preserve useful cache entries
// when the backend returns only the jobs that have changed.
export function mergeSummaryRecords(current, incoming, keys) {
  const allowed = new Set(keys);
  const result = Object.fromEntries(Object.entries(current).filter(([key]) => allowed.has(key)));
  for (const record of incoming || []) {
    if (allowed.has(record.key)) result[record.key] = record;
  }
  return result;
}

export function summaryProgress(items, records) {
  const counts = { ready: 0, running: 0, pending: 0, error: 0, total: items.length };
  for (const item of items) {
    const status = (records[summaryKey(item)] || item.summary)?.status || 'pending';
    counts[status in counts && status !== 'total' ? status : 'pending']++;
  }
  return counts;
}
