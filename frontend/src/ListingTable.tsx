import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { Property } from './types'
import { LABELS } from './labels'
import { STATUS_OPTIONS, displayStatus, MAX_COMPARE_ITEMS } from './constants'
import { formatPriceFull } from './utils'

export type ColKey =
  | 'name' | 'price' | 'status' | 'address' | 'bedrooms' | 'bathrooms'
  | 'sqft' | 'year_built' | 'land_sqft' | 'parking_garage' | 'ac'
  | 'monthly_total' | 'hoa_monthly' | 'property_tax' | 'skytrain' | 'rental_income'
  | 'subway_walk_min' | 'community_center_walk_min' | 'library_walk_min'

export type CompareDirection = 'higher' | 'lower'
export type ColDef = {
  key: ColKey
  label: string
  render: (p: Property) => React.ReactNode
  /** Present for columns whose values can be ranked across compared listings (see ListingComparison). */
  compare?: { direction: CompareDirection; value: (p: Property) => number | null }
}

export const ALL_COLUMNS: ColDef[] = [
  { key: 'name', label: 'Name', render: p => p.title },
  { key: 'price', label: 'Price', render: p => formatPriceFull(p.price, p.price_currency) ?? '—', compare: { direction: 'lower', value: p => p.price ?? null } },
  { key: 'status', label: 'Status', render: p => displayStatus(p.status) || '—' },
  { key: 'address', label: 'Address', render: p => [p.street_address, p.city].filter(Boolean).join(', ') || '—' },
  { key: 'bedrooms', label: 'Beds', render: p => p.bedrooms ?? '—', compare: { direction: 'higher', value: p => p.bedrooms ?? null } },
  { key: 'bathrooms', label: 'Baths', render: p => p.bathrooms ?? '—', compare: { direction: 'higher', value: p => p.bathrooms ?? null } },
  { key: 'sqft', label: LABELS.LIVING_AREA, render: p => p.sqft?.toLocaleString() ?? '—', compare: { direction: 'higher', value: p => p.sqft ?? null } },
  { key: 'year_built', label: LABELS.YEAR_BUILT, render: p => p.year_built ?? '—', compare: { direction: 'higher', value: p => p.year_built ?? null } },
  { key: 'land_sqft', label: LABELS.LOT_SIZE, render: p => p.land_sqft?.toLocaleString() ?? '—', compare: { direction: 'higher', value: p => p.land_sqft ?? null } },
  { key: 'parking_garage', label: LABELS.GARAGE, render: p => p.parking_garage ?? '—', compare: { direction: 'higher', value: p => p.parking_garage ?? null } },
  { key: 'ac', label: LABELS.AIR_CONDITIONING, render: p => p.ac === null ? '—' : p.ac ? 'Yes' : 'No', compare: { direction: 'higher', value: p => p.ac === null ? null : (p.ac ? 1 : 0) } },
  { key: 'monthly_total', label: 'Monthly Total', render: p => p.monthly_total ? `$${p.monthly_total.toLocaleString()}` : '—', compare: { direction: 'lower', value: p => p.monthly_total ?? null } },
  { key: 'hoa_monthly', label: 'HOA', render: p => p.hoa_monthly ? `$${p.hoa_monthly.toLocaleString()}` : '—', compare: { direction: 'lower', value: p => p.hoa_monthly ?? null } },
  { key: 'property_tax', label: 'Tax/yr', render: p => p.property_tax ? `$${p.property_tax.toLocaleString()}` : '—', compare: { direction: 'lower', value: p => p.property_tax ?? null } },
  { key: 'skytrain', label: 'Skytrain', render: p => p.skytrain_station ? `${p.skytrain_station} (${p.skytrain_walk_min ?? '?'} min)` : '—', compare: { direction: 'lower', value: p => p.skytrain_walk_min ?? null } },
  { key: 'rental_income', label: 'Potential Rental Income', render: p => p.rental_income ? `$${p.rental_income.toLocaleString()}/mo` : '—', compare: { direction: 'higher', value: p => p.rental_income ?? null } },
  { key: 'subway_walk_min', label: 'Subway (walk min)', render: p => p.skytrain_walk_min != null ? `${p.skytrain_walk_min} min` : '—', compare: { direction: 'lower', value: p => p.skytrain_walk_min ?? null } },
  { key: 'community_center_walk_min', label: 'Community Center (walk min)', render: p => p.community_center_walk_min != null ? `${p.community_center_walk_min} min` : '—', compare: { direction: 'lower', value: p => p.community_center_walk_min ?? null } },
  { key: 'library_walk_min', label: 'Library (walk min)', render: p => p.library_walk_min != null ? `${p.library_walk_min} min` : '—', compare: { direction: 'lower', value: p => p.library_walk_min ?? null } },
]

export const DEFAULT_COLS: ColKey[] = [
  'name', 'price', 'monthly_total', 'status', 'bedrooms', 'bathrooms', 'sqft', 'land_sqft', 'rental_income',
  'subway_walk_min', 'community_center_walk_min', 'library_walk_min',
]

type SortKey = 'status' | 'monthly_total'
type SortDir = 'asc' | 'desc'

const STATUS_RANK = Object.fromEntries(STATUS_OPTIONS.map((s, i) => [s, i]))

function sortRows(rows: Property[], key: SortKey, dir: SortDir): Property[] {
  return [...rows].sort((a, b) => {
    let cmp = 0
    if (key === 'status') {
      const ra = STATUS_RANK[a.status] ?? 99
      const rb = STATUS_RANK[b.status] ?? 99
      cmp = ra - rb
    } else if (key === 'monthly_total') {
      const va = a.monthly_total ?? Infinity
      const vb = b.monthly_total ?? Infinity
      cmp = va - vb
    }
    return dir === 'asc' ? cmp : -cmp
  })
}

export function ListingTable({ rows, cols, selectedIds, onToggleSelect }: { rows: Property[]; cols: ColDef[]; selectedIds: Set<number>; onToggleSelect: (id: number) => void }) {
  const navigate = useNavigate()
  const [sortKey, setSortKey] = useState<SortKey>('status')
  const [sortDir, setSortDir] = useState<SortDir>('asc')

  function handleSort(key: SortKey) {
    if (sortKey === key) {
      setSortDir(d => d === 'asc' ? 'desc' : 'asc')
    } else {
      setSortKey(key)
      setSortDir('asc')
    }
  }

  const sorted = sortRows(rows, sortKey, sortDir)
  const sortIcon = (key: SortKey) => sortKey === key ? (sortDir === 'asc' ? ' ▲' : ' ▼') : ' ⇅'
  const atLimit = selectedIds.size >= MAX_COMPARE_ITEMS

  return (
    <div className="table-wrap">
      <table className="listings-table">
        <thead>
          <tr>
            <th className="table-check-col" />
            {cols.map(c => {
              const sortable = c.key === 'status' || c.key === 'monthly_total'
              return (
                <th
                  key={c.key}
                  onClick={sortable ? () => handleSort(c.key as SortKey) : undefined}
                  style={sortable ? { cursor: 'pointer', userSelect: 'none' } : undefined}
                >
                  {c.label}{sortable ? <span style={{ opacity: sortKey === c.key ? 1 : 0.35 }}>{sortIcon(c.key as SortKey)}</span> : null}
                </th>
              )
            })}
          </tr>
        </thead>
        <tbody>
          {sorted.map(p => (
            <tr key={p.id} onClick={() => navigate(`/property/${p.id}`)} className="table-row">
              <td
                className="table-check-col"
                onClick={e => e.stopPropagation()}
                title={!selectedIds.has(p.id) && atLimit ? `You can compare up to ${MAX_COMPARE_ITEMS} listings at once` : undefined}
              >
                <input
                  type="checkbox"
                  checked={selectedIds.has(p.id)}
                  disabled={!selectedIds.has(p.id) && atLimit}
                  onChange={() => onToggleSelect(p.id)}
                  aria-label="Select for comparison"
                />
              </td>
              {cols.map(c => <td key={c.key}>{c.render(p)}</td>)}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

