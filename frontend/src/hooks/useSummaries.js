import { useEffect, useState } from 'react';
import { api } from '../api';
import { mergeSummaryRecords, summaryKey, summaryProgress } from './summaryState';

export default function useSummaries(items) {
  const [state, setState] = useState({ signature: '', records: {}, queue: null, error: '' });
  const [revision, setRevision] = useState(0);
  const signature = JSON.stringify(items.map(summaryKey));
  useEffect(() => {
    const keys = JSON.parse(signature);
    let active = true;
    let timer;
    async function poll() {
      const results = await Promise.allSettled([api.summaryRecords(keys), api.summaryStatus()]);
      if (!active) return;
      const [records, queue] = results;
      setState((previous) => ({
        signature,
        records: mergeSummaryRecords(previous.signature === signature ? previous.records : {}, records.status === 'fulfilled' ? records.value : [], keys),
        queue: queue.status === 'fulfilled' ? queue.value : null,
        error: records.status === 'rejected' ? String(records.reason) : queue.status === 'rejected' ? String(queue.reason) : '',
      }));
      const terminal = records.status === 'fulfilled' && keys.every((key) => records.value.some((record) => record.key === key && ['ready', 'error'].includes(record.status)));
      const queueIdle = queue.status === 'fulfilled' && !queue.value.pending && !queue.value.running;
      if (!terminal || !queueIdle) timer = setTimeout(poll, records.status === 'rejected' || queue.status === 'rejected' ? 10000 : 2500);
    }
    if (keys.length) poll();
    return () => { active = false; clearTimeout(timer); };
  }, [signature, revision]);
  const records = state.signature === signature ? state.records : {};
  const progress = summaryProgress(items, records);
  async function retry(keys) {
    await api.retrySummaries(keys);
    setRevision((value) => value + 1);
  }
  return { records, progress, queue: state.signature === signature ? state.queue : null, error: state.signature === signature ? state.error : '', retry };
}
