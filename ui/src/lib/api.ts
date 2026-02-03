const BASE_URL = '/api/v1';

async function fetchJSON<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  return response.json() as Promise<T>;
}

export interface RegistryStats {
  registry_url: string;
  counts: RegistryCounts;
}

export interface RegistryCounts {
  attributes: number;
  metrics: number;
  spans: number;
  events: number;
  entities: number;
  attribute_groups: number;
}

export interface AttributeResponse {
  key: string;
  stability?: StabilityFilter;
  deprecated?: {
    note?: string;
    renamed_to?: string;
  } | boolean;
  brief?: string;
  note?: string;
  type: string | { members: Array<{ value?: string; id?: string; brief?: string }> };
  examples?: unknown[];
}

export interface MetricAttribute {
  key: string;
  'r#type': string | { members: Array<{ value?: string; id?: string; brief?: string }> };
  brief?: string;
  requirement_level:
    | 'required'
    | 'recommended'
    | 'opt_in'
    | { conditionally_required?: string };
}

export interface MetricResponse {
  name: string;
  stability?: StabilityFilter;
  deprecated?: {
    note?: string;
    renamed_to?: string;
  } | boolean;
  brief?: string;
  note?: string;
  instrument: string;
  unit: string;
  attributes?: MetricAttribute[];
  entity_associations?: string[];
}

export interface SpanAttribute {
  key: string;
  type: string | { members: Array<{ value?: string; id?: string; brief?: string }> };
  brief?: string;
  requirement_level:
    | 'required'
    | 'recommended'
    | 'opt_in'
    | { conditionally_required?: string };
  sampling_relevant?: boolean;
}

export interface SpanResponse {
  type: string;
  stability?: StabilityFilter;
  deprecated?: {
    note?: string;
    renamed_to?: string;
  } | boolean;
  brief?: string;
  note?: string;
  kind?: string;
  attributes?: SpanAttribute[];
}

export interface EventAttribute {
  key: string;
  type: string;
  brief?: string;
  requirement_level:
    | 'required'
    | 'recommended'
    | 'opt_in'
    | { conditionally_required?: string };
}

export interface EventResponse {
  name: string;
  stability?: StabilityFilter;
  deprecated?: {
    note?: string;
    renamed_to?: string;
  } | boolean;
  brief?: string;
  note?: string;
  attributes?: EventAttribute[];
}

export interface EntityAttribute {
  key: string;
  'r#type': string | { members: Array<{ value?: string; id?: string; brief?: string }> };
  brief?: string;
}

export interface EntityResponse {
  type: string;
  stability?: StabilityFilter;
  deprecated?: {
    note?: string;
    renamed_to?: string;
  } | boolean;
  brief?: string;
  note?: string;
  identity?: EntityAttribute[];
  description?: EntityAttribute[];
}

export interface SchemaProperty {
  type?: string | string[];
  $ref?: string;
  items?: SchemaProperty;
  additionalProperties?: SchemaProperty;
  enum?: unknown[];
  description?: string;
  required?: string[];
}

export interface SchemaDefinition {
  description?: string;
  type?: string;
  properties?: Record<string, SchemaProperty>;
  required?: string[];
  enum?: unknown[];
  oneOf?: SchemaProperty[];
  anyOf?: SchemaProperty[];
}

export interface SchemaResponse {
  title?: string;
  description?: string;
  type?: string;
  properties?: Record<string, SchemaProperty>;
  required?: string[];
  oneOf?: SchemaProperty[];
  anyOf?: SchemaProperty[];
  definitions?: Record<string, SchemaDefinition>;
}

export interface SearchResult {
  result_type: 'attribute' | 'metric' | 'span' | 'event' | 'entity';
  score: number;
  stability?: Exclude<StabilityFilter, null>;
  deprecated?: boolean;
  brief?: string;
  key?: string;
  name?: string;
  type?: unknown;
  instrument?: string;
  unit?: string;
  kind?: string;
}

export interface SearchResponse {
  query: string | null;
  total: number;
  count: number;
  offset: number;
  results: SearchResult[];
}

export type StabilityFilter =
  | 'stable'
  | 'development'
  | 'alpha'
  | 'beta'
  | 'release_candidate'
  | 'deprecated'
  | null;
export type TypeFilter = 'all' | 'attribute' | 'metric' | 'span' | 'event' | 'entity';

export async function getRegistryStats(): Promise<RegistryStats> {
  return fetchJSON<RegistryStats>(`${BASE_URL}/registry/stats`);
}

export async function getAttribute(key: string): Promise<AttributeResponse> {
  return fetchJSON<AttributeResponse>(
    `${BASE_URL}/registry/attribute/${encodeURIComponent(key)}`
  );
}

export async function getMetric(name: string): Promise<MetricResponse> {
  return fetchJSON<MetricResponse>(`${BASE_URL}/registry/metric/${encodeURIComponent(name)}`);
}

export async function getSpan(type: string): Promise<SpanResponse> {
  return fetchJSON<SpanResponse>(`${BASE_URL}/registry/span/${encodeURIComponent(type)}`);
}

export async function getEvent(name: string): Promise<EventResponse> {
  return fetchJSON<EventResponse>(`${BASE_URL}/registry/event/${encodeURIComponent(name)}`);
}

export async function getEntity(type: string): Promise<EntityResponse> {
  return fetchJSON<EntityResponse>(`${BASE_URL}/registry/entity/${encodeURIComponent(type)}`);
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

export async function getSchema(name: string): Promise<SchemaResponse> {
  return fetchJSON<SchemaResponse>(`${BASE_URL}/schema/${encodeURIComponent(name)}`);
}
