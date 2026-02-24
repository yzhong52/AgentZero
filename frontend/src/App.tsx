import { useEffect, useState } from 'react'
import './App.css'
import { ListingGrid } from './ListingGrid'
import { ListingTable, ALL_COLUMNS, DEFAULT_COLS } from './ListingTable'
import type { ColKey } from './ListingTable'

export type ImageEntry = {
  id: number
  url: string
  created_at: string
}

export type Property = {
  id: number
  redfin_url: string | null
  realtor_url: string | null
  rew_url: string | null
  zillow_url: string | null
  title: string
  description: string
  price: number | null
  price_currency: string | null
  offer_price: number | null
  street_address: string | null
  city: string | null
  region: string | null
  postal_code: string | null
  country: string | null
  bedrooms: number | null
  bathrooms: number | null
  sqft: number | null
  year_built: number | null
  lat: number | null
  lon: number | null
  images: ImageEntry[]
  created_at: string
  updated_at: string | null
  notes: string | null
  parking_garage: number | null
  parking_covered: number | null
  parking_open: number | null
  land_sqft: number | null
  property_tax: number | null
  skytrain_station: string | null
  skytrain_walk_min: number | null
  radiant_floor_heating: boolean | null
  ac: boolean | null
  down_payment_pct: number | null
  mortgage_interest_rate: number | null
  amortization_years: number | null
  mortgage_monthly: number | null
  hoa_monthly: number | null
  monthly_total: number | null
  monthly_cost: number | null
  has_rental_suite: boolean | null
  rental_income: number | null
  status: string
  school_elementary: string | null
  school_elementary_rating: number | null
  school_middle: string | null
  school_middle_rating: number | null
  school_secondary: string | null
  school_secondary_rating: number | null
}

export const STATUS_OPTIONS = ['Interested', 'Buyable', 'Pass'] as const
export type StatusOption = typeof STATUS_OPTIONS[number]

export const STATUS_COLORS: Record<string, string> = {
  Interested: '#4f46e5',
  Buyable: '#16a34a',
  Pass: '#9ca3af',
}


function App() {
  const [url, setUrl] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [savedMsg, setSavedMsg] = useState<string | null>(null)
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

  async function handleSave(e: React.FormEvent) {
    e.preventDefault()
    setError(null)
    setSavedMsg(null)
    if (!url.trim()) return setError('Please enter a URL')
    setSaving(true)
    try {
      const resp = await fetch('/api/listings', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ urls: [url.trim()] }),
      })
      if (!resp.ok) throw new Error(await resp.text())
      const saved: Property = await resp.json()
      setSavedMsg(`Saved: ${saved.title || saved.redfin_url || saved.realtor_url || saved.zillow_url}`)
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
      {savedMsg && <div className="message success">{savedMsg}</div>}

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
            <ListingGrid rows={listings} />
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
