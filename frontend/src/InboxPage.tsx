import { useEffect, useState, useCallback } from 'react'
import { useNavigate, useSearchParams } from 'react-router-dom'
import type { Property, SearchProfile } from './types'
import { STATUS_COLORS, HUMAN_PENDING_STATUS, displayStatus } from './constants'
import { formatPriceCompact } from './utils'
import { PropertyDetailContent } from './PropertyDetail'
import './App.css'

// the inbox is a simple triage tool; the only statuses we offer here are
// 'Interested' or 'Pass'.  keeping the list in a constant makes it easier to
// modify later if requirements change.
const INBOX_ACTION_STATUSES = ['Interested', 'Pass'] as const
const INBOX_ACTION_ICONS: Record<typeof INBOX_ACTION_STATUSES[number], string> = {
  Interested: '✓',
  Pass: '✕',
}

export function InboxPage() {
  const navigate = useNavigate()
  const [searchParams] = useSearchParams()
  const [searchProfiles, setSearchProfiles] = useState<SearchProfile[]>([])
  const [listings, setListings] = useState<Property[]>([])
  const [profileFilter, setProfileFilter] = useState<number | null>(() => {
    const p = searchParams.get('profile')
    return p ? Number(p) : null
  })
  const [selectedId, setSelectedId] = useState<number | null>(null)
  const [dismissing, setDismissing] = useState<Set<number>>(new Set())
  const [loading, setLoading] = useState(true)

  useEffect(() => {
    async function load() {
      setLoading(true)
      try {
        const searchResp = await fetch('/api/search-profiles')
        if (!searchResp.ok) return
        const allSearchProfiles: SearchProfile[] = await searchResp.json()
        setSearchProfiles(allSearchProfiles)

        const results = await Promise.all(
          allSearchProfiles.map(s =>
            fetch(`/api/listings?search_profile_id=${s.id}`)
              .then(r => r.ok ? r.json() : [])
              .catch(() => [])
          )
        )
        const pending = (results.flat() as Property[])
          .filter(p => p.status === HUMAN_PENDING_STATUS)
          .sort((a, b) => new Date(b.created_at).getTime() - new Date(a.created_at).getTime())
        setListings(pending)
        const initialFilter = searchParams.get('profile') ? Number(searchParams.get('profile')) : null
        const firstVisible = initialFilter === null
          ? pending[0]
          : pending.find(p => p.search_profile_id === initialFilter) ?? pending[0]
        if (firstVisible) setSelectedId(firstVisible.id)
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [])

  const assign = useCallback(async (id: number, status: string) => {
    const visible = profileFilter === null
      ? listings
      : listings.filter(p => p.search_profile_id === profileFilter)
    const idx = visible.findIndex(p => p.id === id)
    const next =
      visible.find((p, i) => i > idx && !dismissing.has(p.id)) ??
      visible.find((p, i) => i < idx && !dismissing.has(p.id))
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
  }, [listings, dismissing, profileFilter])

  const visibleListings = profileFilter === null
    ? listings
    : listings.filter(p => p.search_profile_id === profileFilter)

  // Auto-select first visible item when filter changes
  useEffect(() => {
    if (visibleListings.length > 0 && !visibleListings.find(p => p.id === selectedId)) {
      setSelectedId(visibleListings[0].id)
    }
  }, [profileFilter])

  const profileCounts = Object.fromEntries(
    searchProfiles.map(s => [s.id, listings.filter(p => p.search_profile_id === s.id).length])
  )

  const selected = visibleListings.find(p => p.id === selectedId) ?? null
  const searchMap = Object.fromEntries(searchProfiles.map(s => [s.id, s.title]))

  return (
    <div className="inbox-page">
      <div className="inbox-sticky-header">
        <div className="inbox-nav">
          <button className="back-btn" onClick={() => navigate('/')}>
            <svg width="7" height="12" viewBox="0 0 7 12" fill="none" aria-hidden="true">
              <path d="M6 1L1 6l5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
            </svg>
            Back
          </button>
          <div className="inbox-nav-title">
            Inbox
            {visibleListings.length > 0 && <span className="inbox-nav-count">{visibleListings.length}</span>}
          </div>
        </div>

        {!loading && searchProfiles.length > 1 && (
          <div className="inbox-profile-filters">
            <button
              className={`inbox-profile-pill${profileFilter === null ? ' active' : ''}`}
              onClick={() => setProfileFilter(null)}
            >All <span className="inbox-profile-pill-count">{listings.length}</span></button>
            {searchProfiles.map(s => (
              <button
                key={s.id}
                className={`inbox-profile-pill${profileFilter === s.id ? ' active' : ''}`}
                onClick={() => setProfileFilter(profileFilter === s.id ? null : s.id)}
              >{s.title} <span className="inbox-profile-pill-count">{profileCounts[s.id] ?? 0}</span></button>
            ))}
          </div>
        )}

        {!loading && visibleListings.length > 0 && (
          <div className="inbox-list">
            {visibleListings.map(p => {
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
                    <div className="inbox-item-price">
                      {formatPriceCompact(p.price) ?? '—'}
                      {p.source_status && <span className="inbox-source-status">{p.source_status}</span>}
                    </div>
                    {p.street_address && <div className="inbox-item-address">{p.street_address}</div>}
                    {p.search_profile_id != null && searchMap[p.search_profile_id] && (
                      <div className="inbox-item-search">{searchMap[p.search_profile_id]}</div>
                    )}
                  </div>
                </div>
              )
            })}
          </div>
        )}
      </div>

      {loading ? (
        <div className="loading">Loading…</div>
      ) : visibleListings.length === 0 ? (
        <div className="inbox-empty">
          <div className="inbox-empty-icon">✓</div>
          <div className="inbox-empty-title">{profileFilter === null ? 'All caught up' : 'Nothing here'}</div>
          <div className="inbox-empty-sub">{profileFilter === null ? 'No properties waiting for review' : 'No pending properties in this profile'}</div>
        </div>
      ) : selected && (
        <PropertyDetailContent
          key={selected.id}
          initialProperty={selected}
          embedded
          onAfterDelete={() => {
            setListings(prev => prev.filter(p => p.id !== selected.id))
            setSelectedId(null)
          }}
        />
      )}

      {!loading && selected && (
        <div className="inbox-float-actions">
          {INBOX_ACTION_STATUSES.map(s => (
            <button
              key={s}
              className="inbox-action-btn"
              style={{ '--btn-color': STATUS_COLORS[s] } as React.CSSProperties}
              onClick={() => assign(selected.id, s)}
              disabled={dismissing.has(selected.id)}
            >
              {INBOX_ACTION_ICONS[s]} {displayStatus(s)}
            </button>
          ))}
        </div>
      )}
    </div>
  )
}
