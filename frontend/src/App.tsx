import { useEffect, useState } from 'react'
import './App.css'
import { ListingGrid } from './ListingGrid'
import { ListingTable, ALL_COLUMNS, DEFAULT_COLS } from './ListingTable'
import type { ColKey } from './ListingTable'
import { STATUS_OPTIONS, STATUS_COLORS } from './constants'
import type { StatusOption } from './constants'
import type { Property } from './types'

function App() {
  const [url, setUrl] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [listings, setListings] = useState<Property[]>([])
  const [statusFilter, setStatusFilter] = useState<Set<StatusOption>>(new Set(['Interested', 'Buyable']))
  const [viewMode, setViewMode] = useState<'grid' | 'table'>('grid')
  const [visibleCols, setVisibleCols] = useState<Set<ColKey>>(new Set(DEFAULT_COLS))
  const [colPickerOpen, setColPickerOpen] = useState(false)

  async function fetchListings(filter?: Set<StatusOption>) {
    const active = filter ?? statusFilter
    const qs = active.size > 0 ? '?status=' + [...active].join(',') : ''
    try {
      const resp = await fetch(`/api/listings${qs}`)
      if (resp.ok) setListings(await resp.json())
    } catch {
      // non-fatal
    }
  }

  useEffect(() => { fetchListings() }, [])

  function toggleStatus(s: StatusOption) {
    setStatusFilter(prev => {
      const next = new Set(prev)
      next.has(s) ? next.delete(s) : next.add(s)
      fetchListings(next)
      return next
    })
  }

  const [savedInfo, setSavedInfo] = useState<{ id: number; title: string } | null>(null)
  const [dupInfo, setDupInfo] = useState<{ id: number; title: string; mls: string | null } | null>(null)

  async function handleSave(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setSavedInfo(null)
    setDupInfo(null)
    if (!url.trim()) return setError('Please enter a URL')
    setSaving(true)
    try {
      const resp = await fetch('/api/listings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ urls: [url.trim()] }),
      })
      if (!resp.ok) {
        const text = await resp.text()
        if (resp.status === 409) {
          try {
            const body = JSON.parse(text)
            if (body.duplicate) {
              setDupInfo({ id: body.existing_id, title: body.existing_title, mls: body.mls_number })
              return
            }
          } catch { /* fall through to generic error */ }
        }
        throw new Error(text)
      }
      const saved: Property = await resp.json()
      setSavedInfo({ id: saved.id, title: saved.title || saved.redfin_url || saved.realtor_url || saved.zillow_url || `Listing #${saved.id}` })
      setUrl('')
      await fetchListings()
    } catch (err: any) {
      setError(err?.message || String(err))
    } finally {
      setSaving(false)
    }
  }

  return (
    <div className="app-root">
      <h1>Agent Zero</h1>

      <form className="form-wrap">
        <div className="input-row">
          <input
            type="url"
            placeholder="Redfin, rew.ca, or Zillow URL"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <button type="submit" onClick={handleSave} disabled={saving}>
            {saving ? 'Saving…' : 'Save'}
          </button>
        </div>
      </form>

      {error && <div className="message error">{error}</div>}
      {dupInfo && (
        <div className="message success">
          Already saved{dupInfo.mls ? ` (MLS ${dupInfo.mls})` : ''}: <a href={`/property/${dupInfo.id}`}>{dupInfo.title || `Listing #${dupInfo.id}`}</a>
        </div>
      )}
      {savedInfo && (
        <div className="message success">
          Saved: <a href={`/property/${savedInfo.id}`}>{savedInfo.title}</a>
        </div>
      )}

      {listings.length > 0 && (
        <section className="listings-section">
          <div className="listings-header">
            <h2>Saved Listings ({listings.length})</h2>

            <div className="status-filter">
              {STATUS_OPTIONS.map((s) => (
                <button
                  key={s}
                  className={`filter-btn${statusFilter.has(s) ? ' active' : ''}`}
                  onClick={() => toggleStatus(s)}
                  style={statusFilter.has(s) ? { background: STATUS_COLORS[s], color: '#fff', borderColor: STATUS_COLORS[s] } : {}}
                >
                  {s}
                </button>
              ))}
            </div>

            <div className="view-controls">
              <button className={`view-btn${viewMode === 'grid' ? ' active' : ''}`} onClick={() => setViewMode('grid')}>Grid</button>
              <button className={`view-btn${viewMode === 'table' ? ' active' : ''}`} onClick={() => setViewMode('table')}>Table</button>
              {viewMode === 'table' && (
                <div className="col-picker-wrap">
                  <button className="view-btn" onClick={() => setColPickerOpen(o => !o)}>Columns ▾</button>
                  {colPickerOpen && (
                    <div className="col-picker-dropdown">
                      {ALL_COLUMNS.map(c => (
                        <label key={c.key} className="col-picker-item">
                          <input
                            type="checkbox"
                            checked={visibleCols.has(c.key)}
                            onChange={() => setVisibleCols(prev => {
                              const next = new Set(prev)
                              next.has(c.key) ? next.delete(c.key) : next.add(c.key)
                              return next
                            })}
                          />
                          {c.label}
                        </label>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </div>

          {viewMode === 'grid' ? (
            <ListingGrid rows={[...listings].sort((a, b) => {
              const ra = STATUS_OPTIONS.indexOf(a.status as any)
              const rb = STATUS_OPTIONS.indexOf(b.status as any)
              return (ra === -1 ? 99 : ra) - (rb === -1 ? 99 : rb)
            })} />
          ) : (
            <ListingTable rows={listings} cols={ALL_COLUMNS.filter(c => visibleCols.has(c.key))} />
          )}
        </section>
      )}

      {listings.length === 0 && (
        <p className="empty">No listings saved yet. Paste a property URL above and click Save.</p>
      )}
    </div>
  )
}

export default App
