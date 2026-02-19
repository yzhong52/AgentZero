import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import './App.css'

export type ImageEntry = {
  id: number
  url: string
  created_at: string
}

export type Property = {
  id: number
  url: string
  title: string
  description: string
  price: number | null
  price_currency: string | null
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
  mortgage_monthly: number | null
  hoa_monthly: number | null
  monthly_total: number | null
  has_rental_suite: boolean | null
  rental_income: number | null
  status: string | null
  nickname: string | null
}

export const STATUS_OPTIONS = ['Interested', 'Buyable', 'Pass'] as const
export type StatusOption = typeof STATUS_OPTIONS[number]

export const STATUS_COLORS: Record<string, string> = {
  Interested: '#4f46e5',
  Buyable: '#16a34a',
  Pass: '#9ca3af',
}

function StatusBadge({ status }: { status: string | null }) {
  if (!status) return null
  return (
    <span className="status-badge" style={{ background: STATUS_COLORS[status] ?? '#888' }}>
      {status}
    </span>
  )
}

function formatPrice(price: number | null, currency: string | null) {
  if (price == null) return null
  return new Intl.NumberFormat('en-CA', {
    style: 'currency',
    currency: currency ?? 'CAD',
    maximumFractionDigits: 0,
  }).format(price)
}

function ListingCard({ p }: { p: Property }) {
  const navigate = useNavigate()
  const img = p.images[0]?.url
  const address = [p.street_address, p.city, p.region, p.postal_code]
    .filter(Boolean)
    .join(', ')

  return (
    <button
      className="listing-card"
      onClick={() => navigate(`/property/${p.id}`)}
      type="button"
    >
      {img && <img src={img} alt={p.title} className="listing-img" />}
      <div className="listing-body">
        <div className="listing-price-row">
          <div className="listing-price">{formatPrice(p.price, p.price_currency)}</div>
          <StatusBadge status={p.status} />
        </div>
        <div className="listing-address">{address || p.url}</div>
        <div className="listing-stats">
          {p.bedrooms != null && <span>{p.bedrooms} bd</span>}
          {p.bathrooms != null && <span>{p.bathrooms} ba</span>}
          {p.sqft != null && <span>{p.sqft.toLocaleString()} sqft</span>}
          {p.year_built != null && <span>Built {p.year_built}</span>}
        </div>
        <div className="listing-title">{p.nickname ?? p.title}</div>
      </div>
    </button>
  )
}

function App() {
  const [url, setUrl] = useState('')
  const [saving, setSaving] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [savedMsg, setSavedMsg] = useState<string | null>(null)
  const [listings, setListings] = useState<Property[]>([])
  const [statusFilter, setStatusFilter] = useState<string>('All')

  async function fetchListings() {
    try {
      const resp = await fetch('/api/listings')
      if (resp.ok) setListings(await resp.json())
    } catch {
      // non-fatal
    }
  }

  useEffect(() => { fetchListings() }, [])

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
        body: JSON.stringify({ url: url.trim() }),
      })
      if (!resp.ok) throw new Error(await resp.text())
      const saved: Property = await resp.json()
      setSavedMsg(`Saved: ${saved.title || saved.url}`)
      setUrl('')
      fetchListings()
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
            placeholder="https://example.com/listing"
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
              {['All', ...STATUS_OPTIONS].map((s) => (
                <button
                  key={s}
                  className={`filter-btn${statusFilter === s ? ' active' : ''}`}
                  onClick={() => setStatusFilter(s)}
                  style={statusFilter === s && s !== 'All' ? { background: STATUS_COLORS[s], color: '#fff', borderColor: STATUS_COLORS[s] } : {}}
                >
                  {s}
                </button>
              ))}
            </div>
          </div>
          <div className="listings-grid">
            {listings
              .filter((p) => statusFilter === 'All' || p.status === statusFilter)
              .map((p) => <ListingCard key={p.id} p={p} />)}
          </div>
        </section>
      )}

      {listings.length === 0 && (
        <p className="empty">No listings saved yet. Paste a property URL above and click Save.</p>
      )}
    </div>
  )
}

export default App
