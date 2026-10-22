import React from 'react';
import ContentCard from './ContentCard';

// Library and catalog repositories share the same card as skills and models.
export default function RepoCard({ repo, rank, onCloneClick, onOpenFolder, onToggleFavorite, summaryRecord, onRetry, onRetrySummary }) {
  const item = { key: `github:${repo.repo_id}`, title: repo.name, description: repo.description, description_pt: repo.analysis?.description_pt || '', url: repo.url, source: 'GitHub', category: repo.language, repo };
  const record = summaryRecord || null;
  return <ContentCard item={item} record={record} rank={rank} onDownload={() => onCloneClick?.(repo)}
    onOpenFolder={onOpenFolder} onToggleFavorite={onToggleFavorite ? () => onToggleFavorite(repo) : undefined}
    onRetry={onRetry || onRetrySummary} />;
}
