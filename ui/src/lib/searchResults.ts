import type { SearchResult } from './api';

/** Canonical name of a search result (attribute key, metric/event name, span/entity type). */
export function getResultId(result: SearchResult): string {
  if (result.result_type === 'span' || result.result_type === 'entity') {
    return String(result.type ?? '');
  }
  return String(result.key ?? result.name ?? result.type ?? '');
}

/** Route to the detail page for a search result. */
export function getResultLink(result: SearchResult): string {
  switch (result.result_type) {
    case 'attribute':
      return `/attribute/${result.key ?? ''}`;
    case 'metric':
      return `/metric/${result.name ?? ''}`;
    case 'span':
      return `/span/${result.type ?? ''}`;
    case 'event':
      return `/event/${result.name ?? ''}`;
    case 'entity':
      return `/entity/${result.type ?? ''}`;
    default:
      return '#';
  }
}

/** Render an attribute's type field (plain string, enum, or template) as a short label. */
export function formatAttributeType(type: SearchResult['type']): string {
  if (typeof type === 'string') return type;
  if (type && typeof type === 'object') {
    const typed = type as { members?: unknown[]; type?: string };
    if (Array.isArray(typed.members)) return 'enum';
    if (typeof typed.type === 'string') return typed.type;
  }
  return type ? JSON.stringify(type) : '';
}

/** Type-specific metadata shown alongside a result (attribute type, metric instrument, ...). */
export function getResultMeta(result: SearchResult): Array<{ label: string; value: string }> {
  switch (result.result_type) {
    case 'attribute':
      return [{ label: 'Type', value: formatAttributeType(result.type) }];
    case 'metric':
      return [
        { label: 'Instrument', value: result.instrument ?? '-' },
        { label: 'Unit', value: result.unit || '-' },
      ];
    case 'span':
      return [{ label: 'Kind', value: result.kind || '-' }];
    case 'event':
    case 'entity':
    default:
      return [];
  }
}
