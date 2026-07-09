const BASE_URL = '/api/v1';

export interface Location {
  line: number;
  col: number;
}

export interface Source {
  start: Location;
  end?: Location;
}

export interface FilterErrorDetail {
  error: string;
  source?: Source;
}

export class ApiError extends Error {
  details?: FilterErrorDetail[];
  constructor(message: string, details?: FilterErrorDetail[]) {
    super(message);
    this.name = 'ApiError';
    this.details = details;
  }
}

async function fetchJSON<T>(url: string, options?: RequestInit): Promise<T> {
  const response = await fetch(url, options);
  if (!response.ok) {
    let errorMsg = `HTTP error! status: ${response.status}`;
    let details: FilterErrorDetail[] | undefined;
    try {
      const errorData = await response.json();
      if (errorData && typeof errorData === 'object' && 'error' in errorData) {
        errorMsg = String(errorData.error);
        if ('details' in errorData && Array.isArray(errorData.details)) {
          details = errorData.details;
        }
      }
    } catch (e) {
      // Ignore parse error, use fallback response status
    }
    throw new ApiError(errorMsg, details);
  }
  return response.json() as Promise<T>;
}

/** A breakdown map: label -> count. */
export type Breakdown = Record<string, number>;

/** Statistics shared by every signal type (metrics, spans, events, entities, attribute groups). */
export interface CommonSignalStats {
  count: number;
  stability_breakdown: Breakdown;
  deprecated_count: number;
  total_with_note: number;
}

export interface AttributeStats {
  attribute_count: number;
  attribute_type_breakdown: Breakdown;
  stability_breakdown: Breakdown;
  deprecated_count: number;
}

export interface MetricStats {
  common: CommonSignalStats;
  metric_names: string[];
  instrument_breakdown: Breakdown;
  unit_breakdown: Breakdown;
}

export interface SpanStats {
  common: CommonSignalStats;
  span_kind_breakdown: Breakdown;
}

export interface EventStats {
  common: CommonSignalStats;
  event_names: string[];
}

export interface EntityStats {
  common: CommonSignalStats;
  entity_types: string[];
  entity_identity_length_distribution: Breakdown;
}

export interface AttributeGroupStats {
  common: CommonSignalStats;
}

export interface RegistryStatsDetail {
  attributes: AttributeStats;
  metrics: MetricStats;
  spans: SpanStats;
  events: EventStats;
  entities: EntityStats;
  attribute_groups: AttributeGroupStats;
}

/** Full registry statistics — the same data produced by `weaver registry stats`. */
export interface RegistryStats {
  schema_url: string;
  version: string;
  registry: RegistryStatsDetail;
  refinements: Record<string, unknown>;
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
  // A single example is a bare scalar; multiple is an array.
  examples?: unknown[] | unknown;
}

export type EntityAssociation =
  | string
  | { one_of: EntityAssociation[] }
  | { all_of: EntityAssociation[] };

export interface MetricAttribute {
  key: string;
  type: string | { members: Array<{ value?: string; id?: string; brief?: string }> };
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
  entity_associations?: EntityAssociation[];
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
  entity_associations?: EntityAssociation[];
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
  entity_associations?: EntityAssociation[];
}

export interface EntityAttribute {
  key: string;
  type: string | { members: Array<{ value?: string; id?: string; brief?: string }> };
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
  $defs?: Record<string, SchemaDefinition>;
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
  offset: number = 0,
  options?: { signal?: AbortSignal }
): Promise<SearchResponse> {
  const searchParams = new URLSearchParams();
  if (query) searchParams.set('q', query);
  if (type !== 'all') searchParams.set('type', type);
  if (stability) searchParams.set('stability', stability);
  if (limit) searchParams.set('limit', limit.toString());
  if (offset) searchParams.set('offset', offset.toString());
  return fetchJSON<SearchResponse>(`${BASE_URL}/registry/search?${searchParams.toString()}`, {
    signal: options?.signal,
  });
}

export async function getSchema(name: string): Promise<SchemaResponse> {
  return fetchJSON<SchemaResponse>(`${BASE_URL}/schema/${encodeURIComponent(name)}`);
}

export async function filterRegistry(filter: string): Promise<unknown> {
  const searchParams = new URLSearchParams();
  searchParams.set('filter', filter);
  return fetchJSON<unknown>(`${BASE_URL}/registry/filter?${searchParams.toString()}`);
}

