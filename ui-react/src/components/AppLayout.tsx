import { useState, useEffect } from 'react'
import { Outlet, useLocation, useNavigate } from '@tanstack/react-router'

export function AppLayout() {
  const location = useLocation()
  const navigate = useNavigate()
  const [isOpen, setIsOpen] = useState(false)
  const [theme, setTheme] = useState<'light' | 'dark'>('light')

  useEffect(() => {
    const savedTheme = localStorage.getItem('theme') as 'light' | 'dark' | null
    const initialTheme = savedTheme || 'light'
    setTheme(initialTheme)
    document.documentElement.setAttribute('data-theme', initialTheme)
  }, [])

  const toggleTheme = () => {
    const newTheme = theme === 'light' ? 'dark' : 'light'
    setTheme(newTheme)
    localStorage.setItem('theme', newTheme)
    document.documentElement.setAttribute('data-theme', newTheme)
  }

  const isActive = (path: string) => {
    if (path === '/') return location.pathname === '/'
    return location.pathname.startsWith(path)
  }

  const closeSidebar = () => {
    setIsOpen(false)
  }

  return (
    <div className="drawer lg:drawer-open">
      <input
        id="sidebar"
        type="checkbox"
        className="drawer-toggle"
        checked={isOpen}
        onChange={(e) => setIsOpen(e.target.checked)}
      />

      <div className="drawer-content flex flex-col">
        {/* Navbar */}
        <div className="navbar bg-base-200 sticky top-0 z-10">
          <div className="flex-none lg:hidden">
            <label htmlFor="sidebar" className="btn btn-square btn-ghost">
              <svg
                xmlns="http://www.w3.org/2000/svg"
                fill="none"
                viewBox="0 0 24 24"
                className="inline-block w-6 h-6 stroke-current"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M4 6h16M4 12h16M4 18h16"
                />
              </svg>
            </label>
          </div>
          <div className="flex-1">
            <button
              onClick={() => navigate({ to: '/' })}
              className="btn btn-ghost text-xl"
            >
              Weaver
            </button>
          </div>
          <div className="flex-none gap-2">
            <button
              className="btn btn-ghost btn-circle"
              onClick={toggleTheme}
              title="Toggle theme"
            >
              {theme === 'light' ? (
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-6 w-6"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M20.354 15.354A9 9 0 018.646 3.646 9.003 9.003 0 0012 21a9.003 9.003 0 008.354-5.646z"
                  />
                </svg>
              ) : (
                <svg
                  xmlns="http://www.w3.org/2000/svg"
                  className="h-6 w-6"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    strokeWidth={2}
                    d="M12 3v1m0 16v1m9-9h-1M4 12H3m15.364 6.364l-.707-.707M6.343 6.343l-.707-.707m12.728 0l-.707.707M6.343 17.657l-.707.707M16 12a4 4 0 11-8 0 4 4 0 018 0z"
                  />
                </svg>
              )}
            </button>
          </div>
        </div>

        {/* Main content */}
        <main className="flex-1 p-6">
          <Outlet />
        </main>
      </div>

      {/* Sidebar */}
      <div className="drawer-side">
        <label
          htmlFor="sidebar"
          className="drawer-overlay"
          onClick={closeSidebar}
        />
        <ul className="menu p-4 w-64 min-h-full bg-base-200 text-base-content">
          <li className="menu-title">Registry</li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/' })
                closeSidebar()
              }}
              className={isActive('/') && location.pathname === '/' ? 'active' : ''}
            >
              Search
            </button>
          </li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/stats' })
                closeSidebar()
              }}
              className={isActive('/stats') ? 'active' : ''}
            >
              Stats
            </button>
          </li>
          <li className="menu-title mt-4">Schema</li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/schema', search: { schema: 'ForgeRegistryV2' } })
                closeSidebar()
              }}
              className={
                isActive('/schema') && location.pathname.includes('ForgeRegistryV2') ? 'active' : ''
              }
            >
              ForgeRegistryV2
            </button>
          </li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/schema', search: { schema: 'SemconvDefinitionV2' } })
                closeSidebar()
              }}
              className={
                isActive('/schema') && location.pathname.includes('SemconvDefinitionV2')
                  ? 'active'
                  : ''
              }
            >
              SemconvDefinitionV2
            </button>
          </li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/schema', search: { schema: 'LiveCheckSample' } })
                closeSidebar()
              }}
              className={
                isActive('/schema') && location.pathname.includes('LiveCheckSample') ? 'active' : ''
              }
            >
              LiveCheckSample
            </button>
          </li>
          <li className="menu-title mt-4">Developer</li>
          <li>
            <button
              onClick={() => {
                navigate({ to: '/api-docs' })
                closeSidebar()
              }}
              className={isActive('/api-docs') ? 'active' : ''}
            >
              API Documentation
            </button>
          </li>
        </ul>
      </div>
    </div>
  )
}
