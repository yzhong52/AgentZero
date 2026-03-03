import { useEffect, useState, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import type { Property, SavedSearch } from './types'
import { STATUS_OPTIONS, STATUS_COLORS, PENDING_STATUS } from './constants'
import { formatPriceCompact } from './utils'
import './App.css'

export function InboxPage() {
  const navigate = useNavigate()
  const [searches, setSearches] = useState<SavedSearch[]>([])
  const [listings, setListings] = useState<Property[]>([])
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [dismissing, setDismissing] = useState<Set<number>>(new Set())
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function load() {
      setLoading(true)
      try {
        const searchResp = await fetch('/api/searches')
        if (!searchResp.ok) return
        const allSearches: SavedSearch[] = await searchResp.json()
        setSearches(allSearches)

        const results = await Promise.all(
          allSearches.map(s =>
            fetch(`/api/listings?search_criteria_id=${s.id}`)
              .then(r => r.ok ? r.json() : [])
              .catch(() => [])
          )
        )
        const pending = (results.flat() as Property[])
          .filter(p => p.status === PENDING_STATUS)
          .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
        setListings(pending)
        if (pending.length > 0) setSelectedId(pending[0].id)
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [])

  const assign = useCallback(async (id: number, status: string) => {
    const idx = listings.findIndex(p => p.id === id)
    const next =
      listings.find((p, i) => i > idx && !dismissing.has(p.id)) ??
      listings.find((p, i) => i < idx && !dismissing.has(p.id))
    setSelectedId(next?.id ?? null)

    setDismissing(prev => new Set(prev).add(id))
    setTimeout(() => {
      setListings(prev => prev.filter(p => p.id !== id))
      setDismissing(prev => { const s = new Set(prev); s.delete(id); return s })
    }, 300)

    try {
      await fetch(`/api/listings/${id}/details`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status }),
      })
    } catch { /* non-fatal */ }
  }, [listings, dismissing])

  useEffect(() => {
    function handleKey(e: KeyboardEvent) {
      if ((e.target as Element).tagName === 'INPUT' || (e.target as Element).tagName === 'TEXTAREA') return
      const idx = listings.findIndex(p => p.id === selectedId)
      switch (e.key) {
        case 'j': case 'ArrowDown':
          e.preventDefault()
          if (idx < listings.length - 1) setSelectedId(listings[idx + 1].id)
          break
        case 'k': case 'ArrowUp':
          e.preventDefault()
          if (idx > 0) setSelectedId(listings[idx - 1].id)
          break
        case 'b':
          if (selectedId && !dismissing.has(selectedId)) assign(selectedId, 'Buyable')
          break
        case 'i':
          if (selectedId && !dismissing.has(selectedId)) assign(selectedId, 'Interested')
          break
        case 'p':
          if (selectedId && !dismissing.has(selectedId)) assign(selectedId, 'Pass')
          break
      }
    }
    window.addEventListener('keydown', handleKey)
    return () => window.removeEventListener('keydown', handleKey)
  }, [selectedId, listings, dismissing, assign])

  const selected = listings.find(p => p.id === selectedId) ?? null
  const searchMap = Object.fromEntries(searches.map(s => [s.id, s.title]))

  return (
    <div className="inbox-page">
      <div className="inbox-nav">
        <button className="back-btn" onClick={() => navigate('/')}>
          <svg width="7" height="12" viewBox="0 0 7 12" fill="none" aria-hidden="true">
            <path d="M6 1L1 6l5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Back
        </button>
        <div className="inbox-nav-title">
          Inbox
          {listings.length > 0 && <span className="inbox-nav-count">{listings.length}</span>}
        </div>
        <div className="inbox-nav-hint">j / k navigate · b / i / p assign</div>
      </div>

      {loading ? (
        <div className="loading">Loading…</div>
      ) : listings.length === 0 ? (
        <div className="inbox-empty">
          <div className="inbox-empty-icon">✓</div>
          <div className="inbox-empty-title">All caught up</div>
          <div className="inbox-empty-sub">No properties waiting for review</div>
        </div>
      ) : (
        <div className="inbox-body">
          <div className="inbox-list">
            {listings.map(p => {
              const img = p.images[0]?.url
              return (
                <div
                  key={p.id}
                  className={[
                    'inbox-item',
                    p.id === selectedId ? 'selected' : '',
                    dismissing.has(p.id) ? 'dismissing' : '',
                  ].filter(Boolean).join(' ')}
                  onClick={() => setSelectedId(p.id)}
                >
                  <div className="inbox-item-thumb">
                    {img
                      ? <img src={img} alt={p.title} />
                      : <div className="inbox-item-thumb-empty" />
                    }
                  </div>
                  <div className="inbox-item-info">
                    <div className="inbox-item-price">{formatPriceCompact(p.price) ?? '—'}</div>
                    {p.street_address && <div className="inbox-item-address">{p.street_address}</div>}
                    {searchMap[p.search_criteria_id] && (
                      <div className="inbox-item-search">{searchMap[p.search_criteria_id]}</div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>

          {selected && (
            <div className="inbox-detail">
              <div className="inbox-detail-photos">
                {selected.images.length > 0
                  ? <img src={selected.images[0].url} alt={selected.title} />
                  : <div className="inbox-detail-no-photo" />
                }
              </div>
              <div className="inbox-detail-content">
                <div className="inbox-detail-price">{formatPriceCompact(selected.price) ?? '—'}</div>
                {selected.street_address && (
                  <div className="inbox-detail-address">
                    {[selected.street_address, selected.city, selected.region].filter(Boolean).join(', ')}
                  </div>
                )}
                <div className="inbox-detail-stats">
                  {selected.bedrooms != null && <span>{selected.bedrooms} bd</span>}
                  {selected.bathrooms != null && <span>{selected.bathrooms} ba</span>}
                  {selected.sqft != null && <span>{selected.sqft.toLocaleString()} sqft</span>}
                  {selected.year_built != null && <span>Built {selected.year_built}</span>}
                </div>
                {searchMap[selected.search_criteria_id] && (
                  <div className="inbox-detail-search-tag">{searchMap[selected.search_criteria_id]}</div>
                )}
                {selected.description && (
                  <p className="inbox-detail-desc">{selected.description}</p>
                )}
                <div className="inbox-detail-actions">
                  {STATUS_OPTIONS.filter(s => s !== PENDING_STATUS).map(s => (
                    <button
                      key={s}
                      className="inbox-action-btn"
                      style={{ '--btn-color': STATUS_COLORS[s] } as React.CSSProperties}
                      onClick={() => assign(selected.id, s)}
                      disabled={dismissing.has(selected.id)}
                    >
                      {s}
                    </button>
                  ))}
                </div>
                <button className="inbox-view-link" onClick={() => navigate(`/property/${selected.id}`)}>
                  View full details →
                </button>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  )
}
