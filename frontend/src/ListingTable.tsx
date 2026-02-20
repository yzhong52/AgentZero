import { useNavigate } from 'react-router-dom'
import { Property } from './App'

export type ColKey =
  | 'name' | 'price' | 'status' | 'address' | 'bedrooms' | 'bathrooms'
  | 'sqft' | 'year_built' | 'land_sqft' | 'parking_garage' | 'ac'
  | 'monthly_total' | 'hoa_monthly' | 'property_tax' | 'skytrain'

export type ColDef = { key: ColKey; label: string; render: (p: Property) => React.ReactNode }

function formatPrice(price: number | null, currency: string | null) {
  if (price == null) return null
  return new Intl.NumberFormat('en-CA', {
    style: 'currency',
    currency: currency ?? 'CAD',
    maximumFractionDigits: 0,
  }).format(price)
}

export const ALL_COLUMNS: ColDef[] = [
  { key: 'name',           label: 'Name',      render: p => p.nickname ?? p.title },
  { key: 'price',          label: 'Price',     render: p => formatPrice(p.price, p.price_currency) ?? '—' },
  { key: 'status',         label: 'Status',    render: p => p.status ?? '—' },
  { key: 'address',        label: 'Address',   render: p => [p.street_address, p.city].filter(Boolean).join(', ') || '—' },
  { key: 'bedrooms',       label: 'Beds',      render: p => p.bedrooms ?? '—' },
  { key: 'bathrooms',      label: 'Baths',     render: p => p.bathrooms ?? '—' },
  { key: 'sqft',           label: 'Sqft',      render: p => p.sqft?.toLocaleString() ?? '—' },
  { key: 'year_built',     label: 'Year Built',render: p => p.year_built ?? '—' },
  { key: 'land_sqft',      label: 'Land Sqft', render: p => p.land_sqft?.toLocaleString() ?? '—' },
  { key: 'parking_garage', label: 'Garage',    render: p => p.parking_garage ?? '—' },
  { key: 'ac',             label: 'Air Conditioning', render: p => p.ac === null ? '—' : p.ac ? 'Yes' : 'No' },
  { key: 'monthly_total',  label: 'Monthly',   render: p => p.monthly_total ? `$${p.monthly_total.toLocaleString()}` : '—' },
  { key: 'hoa_monthly',    label: 'HOA',       render: p => p.hoa_monthly ? `$${p.hoa_monthly.toLocaleString()}` : '—' },
  { key: 'property_tax',   label: 'Tax/yr',    render: p => p.property_tax ? `$${p.property_tax.toLocaleString()}` : '—' },
  { key: 'skytrain',       label: 'Skytrain',  render: p => p.skytrain_station ? `${p.skytrain_station} (${p.skytrain_walk_min ?? '?'} min)` : '—' },
]

export const DEFAULT_COLS: ColKey[] = ['name', 'price', 'status', 'address', 'bedrooms', 'bathrooms', 'sqft']

export function ListingTable({ rows, cols }: { rows: Property[]; cols: ColDef[] }) {
  const navigate = useNavigate()
  return (
    <div className="table-wrap">
      <table className="listings-table">
        <thead>
          <tr>{cols.map(c => <th key={c.key}>{c.label}</th>)}</tr>
        </thead>
        <tbody>
          {rows.map(p => (
            <tr key={p.id} onClick={() => navigate(`/property/${p.id}`)} className="table-row">
              {cols.map(c => <td key={c.key}>{c.render(p)}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
