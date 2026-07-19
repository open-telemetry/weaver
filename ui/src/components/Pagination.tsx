import { useMemo } from 'react'

interface PaginationProps {
  total: number
  limit: number
  offset: number
  onPageChange: (newOffset: number) => void
}

export function Pagination({ total, limit, offset, onPageChange }: PaginationProps) {
  const totalPages = Math.ceil(total / limit)
  const currentPage = Math.floor(offset / limit) + 1

  const visiblePages = useMemo(() => {
    const maxVisible = 7
    const pages: number[] = []

    if (totalPages <= maxVisible) {
      for (let i = 1; i <= totalPages; i += 1) {
        pages.push(i)
      }
      return pages
    }

    let start = Math.max(1, currentPage - Math.floor(maxVisible / 2))
    const end = Math.min(totalPages, start + maxVisible - 1)

    if (end === totalPages) {
      start = Math.max(1, end - maxVisible + 1)
    }

    for (let i = start; i <= end; i += 1) {
      pages.push(i)
    }

    return pages
  }, [currentPage, totalPages])

  const goToPage = (page: number) => {
    const newOffset = (page - 1) * limit
    onPageChange(newOffset)
  }

  if (totalPages <= 1) return null

  return (
    <div className="join">
      <button
        className="join-item btn btn-sm"
        disabled={currentPage === 1}
        onClick={() => goToPage(currentPage - 1)}
        type="button"
        aria-label="Previous page"
      >
        <span aria-hidden="true">«</span>
      </button>

      {visiblePages[0] > 1 && (
        <>
          <button className="join-item btn btn-sm" onClick={() => goToPage(1)} type="button">
            1
          </button>
          {visiblePages[0] > 2 && (
            <button className="join-item btn btn-sm btn-disabled" type="button">
              <span aria-hidden="true">...</span>
            </button>
          )}
        </>
      )}

      {visiblePages.map((page) => (
        <button
          key={page}
          className={`join-item btn btn-sm${page === currentPage ? ' btn-active' : ''}`}
          onClick={() => goToPage(page)}
          type="button"
        >
          {page}
        </button>
      ))}

      {visiblePages[visiblePages.length - 1] < totalPages && (
        <>
          {visiblePages[visiblePages.length - 1] < totalPages - 1 && (
            <button className="join-item btn btn-sm btn-disabled" type="button">
              <span aria-hidden="true">...</span>
            </button>
          )}
          <button
            className="join-item btn btn-sm"
            onClick={() => goToPage(totalPages)}
            type="button"
          >
            {totalPages}
          </button>
        </>
      )}

      <button
        className="join-item btn btn-sm"
        disabled={currentPage === totalPages}
        onClick={() => goToPage(currentPage + 1)}
        type="button"
        aria-label="Next page"
      >
        <span aria-hidden="true">»</span>
      </button>
    </div>
  )
}
