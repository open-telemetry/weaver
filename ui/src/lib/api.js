const BASE_URL = '/api/v1';

async function fetchJSON(url, options = {}) {
  const response = await fetch(url, options);
  if (!response.ok) {
    throw new Error(`HTTP error! status: ${response.status}`);
  }
  return response.json();
}

export async function getRegistry() {
  return fetchJSON(`${BASE_URL}/registry`);
}

export async function getAttributes(params = {}) {
  const searchParams = new URLSearchParams();
  if (params.stability) searchParams.set('stability', params.stability);
  if (params.limit) searchParams.set('limit', params.limit);
  if (params.offset) searchParams.set('offset', params.offset);
  const query = searchParams.toString();
  return fetchJSON(`${BASE_URL}/attributes${query ? '?' + query : ''}`);
}

export async function getAttribute(key) {
  return fetchJSON(`${BASE_URL}/attributes/${key}`);
}

export async function getMetrics(params = {}) {
  const searchParams = new URLSearchParams();
  if (params.stability) searchParams.set('stability', params.stability);
  if (params.limit) searchParams.set('limit', params.limit);
  if (params.offset) searchParams.set('offset', params.offset);
  const query = searchParams.toString();
  return fetchJSON(`${BASE_URL}/metrics${query ? '?' + query : ''}`);
}

export async function getMetric(name) {
  return fetchJSON(`${BASE_URL}/metrics/${name}`);
}

export async function getSpans(params = {}) {
  const searchParams = new URLSearchParams();
  if (params.stability) searchParams.set('stability', params.stability);
  if (params.limit) searchParams.set('limit', params.limit);
  if (params.offset) searchParams.set('offset', params.offset);
  const query = searchParams.toString();
  return fetchJSON(`${BASE_URL}/spans${query ? '?' + query : ''}`);
}

export async function getSpan(type) {
  return fetchJSON(`${BASE_URL}/spans/${type}`);
}

export async function getEvents(params = {}) {
  const searchParams = new URLSearchParams();
  if (params.stability) searchParams.set('stability', params.stability);
  if (params.limit) searchParams.set('limit', params.limit);
  if (params.offset) searchParams.set('offset', params.offset);
  const query = searchParams.toString();
  return fetchJSON(`${BASE_URL}/events${query ? '?' + query : ''}`);
}

export async function getEvent(name) {
  return fetchJSON(`${BASE_URL}/events/${name}`);
}

export async function getEntities(params = {}) {
  const searchParams = new URLSearchParams();
  if (params.stability) searchParams.set('stability', params.stability);
  if (params.limit) searchParams.set('limit', params.limit);
  if (params.offset) searchParams.set('offset', params.offset);
  const query = searchParams.toString();
  return fetchJSON(`${BASE_URL}/entities${query ? '?' + query : ''}`);
}

export async function getEntity(type) {
  return fetchJSON(`${BASE_URL}/entities/${type}`);
}

export async function search(query, type = 'all', limit = 50) {
  const searchParams = new URLSearchParams();
  searchParams.set('q', query);
  if (type !== 'all') searchParams.set('type', type);
  if (limit) searchParams.set('limit', limit);
  return fetchJSON(`${BASE_URL}/search?${searchParams.toString()}`);
}
