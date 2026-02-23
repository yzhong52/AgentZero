import { useNavigate } from 'react-router-dom'
import { STATUS_COLORS, type Property } from './App'

function formatPrice(price: number | null, currency: string | null) {
  if (price == null) return null
  return new Intl.NumberFormat('en-CA', {
    style: 'currency',
    currency: currency ?? 'CAD',
    maximumFractionDigits: 0,
  }).format(price)
}

function StatusBadge({ status }: { status: string | null }) {
  if (!status) return null
  return (
    <span className="status-badge" style={{ background: STATUS_COLORS[status] ?? '#888' }}>
      {status}
    </span>
  )
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
        <div className="listing-address">{address || p.redfin_url || p.realtor_url}</div>
        <div className="listing-stats">
          {p.bedrooms != null && <span>{p.bedrooms} bd</span>}
          {p.bathrooms != null && <span>{p.bathrooms} ba</span>}
          {p.sqft != null && <span>{p.sqft.toLocaleString()} sqft</span>}
          {p.year_built != null && <span>Built {p.year_built}</span>}
        </div>
        <div className="listing-title">{p.title}</div>
      </div>
    </button>
  )
}

export function ListingGrid({ rows }: { rows: Property[] }) {
  return (
    <div className="listings-grid">
      {rows.map(p => <ListingCard key={p.id} p={p} />)}
    </div>
  )
}
