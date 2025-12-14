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

export async function getAttribute(key) {
  return fetchJSON(`${BASE_URL}/attributes/${key}`);
}

export async function getMetric(name) {
  return fetchJSON(`${BASE_URL}/metrics/${name}`);
}

export async function getSpan(type) {
  return fetchJSON(`${BASE_URL}/spans/${type}`);
}

export async function getEvent(name) {
  return fetchJSON(`${BASE_URL}/events/${name}`);
}

export async function getEntity(type) {
  return fetchJSON(`${BASE_URL}/entities/${type}`);
}

/**
 * Unified search endpoint that handles both text search and browsing.
 *
 * @param {string|null} query - Search query (null or empty for browse mode)
 * @param {string} type - Type filter (all/attribute/metric/span/event/entity)
 * @param {string|null} stability - Stability filter (stable/development/alpha/beta)
 * @param {number} limit - Maximum results
 * @param {number} offset - Pagination offset
 */
export async function search(query = null, type = 'all', stability = null, limit = 50, offset = 0) {
  const searchParams = new URLSearchParams();
  if (query) searchParams.set('q', query);
  if (type !== 'all') searchParams.set('type', type);
  if (stability) searchParams.set('stability', stability);
  if (limit) searchParams.set('limit', limit);
  if (offset) searchParams.set('offset', offset);
  return fetchJSON(`${BASE_URL}/search?${searchParams.toString()}`);
}
