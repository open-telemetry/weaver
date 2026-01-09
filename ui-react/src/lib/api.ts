const BASE_URL = '/api/v1';

async function fetchJSON<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export interface RegistryStats {
  attributes: number;
  metrics: number;
  spans: number;
  events: number;
  entities: number;
}

export interface SearchResult {
  id: string;
  type: 'attribute' | 'metric' | 'span' | 'event' | 'entity';
  brief?: string;
  stability: string;
  deprecated?: boolean;
}

export interface SearchResponse {
  items: SearchResult[];
  total: number;
  offset: number;
  limit: number;
}

export type StabilityFilter = 'stable' | 'development' | 'alpha' | 'beta' | null;
export type TypeFilter = 'all' | 'attribute' | 'metric' | 'span' | 'event' | 'entity';

export async function getRegistryStats(): Promise<RegistryStats> {
  return fetchJSON<RegistryStats>(`${BASE_URL}/registry/stats`);
}

export async function getAttribute(key: string): Promise<unknown> {
  return fetchJSON(`${BASE_URL}/registry/attribute/${encodeURIComponent(key)}`);
}

export async function getMetric(name: string): Promise<unknown> {
  return fetchJSON(`${BASE_URL}/registry/metric/${encodeURIComponent(name)}`);
}

export async function getSpan(type: string): Promise<unknown> {
  return fetchJSON(`${BASE_URL}/registry/span/${encodeURIComponent(type)}`);
}

export async function getEvent(name: string): Promise<unknown> {
  return fetchJSON(`${BASE_URL}/registry/event/${encodeURIComponent(name)}`);
}

export async function getEntity(type: string): Promise<unknown> {
  return fetchJSON(`${BASE_URL}/registry/entity/${encodeURIComponent(type)}`);
}

export async function search(
  query: string | null = null,
  type: TypeFilter = 'all',
  stability: StabilityFilter = null,
  limit: number = 50,
  offset: number = 0
): Promise<SearchResponse> {
  const searchParams = new URLSearchParams();
  if (query) searchParams.set('q', query);
  if (type !== 'all') searchParams.set('type', type);
  if (stability) searchParams.set('stability', stability);
  if (limit) searchParams.set('limit', limit.toString());
  if (offset) searchParams.set('offset', offset.toString());
  return fetchJSON<SearchResponse>(`${BASE_URL}/registry/search?${searchParams.toString()}`);
}
