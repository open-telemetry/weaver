import { useEffect, useState } from 'react'

export function useResourceFetch<T>(id: string, fetcher: (id: string) => Promise<T>) {
  const [data, setData] = useState<T | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    let isMounted = true

    setLoading(true)
    setError(null)
    setData(null)

    fetcher(id)
      .then((responseData) => {
        if (isMounted) {
          setData(responseData)
        }
      })
      .catch((err: unknown) => {
        if (isMounted) {
          setError(err instanceof Error ? err.message : 'Unknown error')
        }
      })
      .finally(() => {
        if (isMounted) {
          setLoading(false)
        }
      })

    return () => {
      isMounted = false
    }
  }, [id, fetcher])

  return { data, error, loading }
}
