import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { STATUS_OPTIONS, STATUS_COLORS } from './constants'
import type { Property } from './types'
import { formatPriceCompact } from './utils'

interface ReviewQueueProps {
  listings: Property[]
  onReviewed: (id: number, status: string) => void
}

export function ReviewQueue({ listings, onReviewed }: ReviewQueueProps) {
  const navigate = useNavigate()
  const [collapsed, setCollapsed] = useState(false)
  const [assigning, setAssigning] = useState<number | null>(null)

  if (listings.length === 0) return null

  async function assign(id: number, status: string) {
    setAssigning(id)
    onReviewed(id, status)
    try {
      await fetch(`/api/listings/${id}/details`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ status }),
      })
    } catch { /* non-fatal — optimistic update already applied */ }
    setAssigning(null)
  }

  return (
    <section className="review-queue">
      <button className="review-queue-header" onClick={() => setCollapsed(c => !c)}>
        <span className="review-queue-badge">{listings.length}</span>
        <span className="review-queue-title">New from Agent Zero</span>
        <span className="review-queue-chevron">{collapsed ? '▸' : '▾'}</span>
      </button>

      {!collapsed && (
        <div className="review-queue-scroll">
          {listings.map(p => {
            const img = p.images[0]?.url
            const address = p.street_address
            return (
              <div key={p.id} className="review-card">
                <div className="review-card-img-wrap">
                  {img
                    ? <img src={img} alt={p.title} className="review-card-img" />
                    : <div className="review-card-img-placeholder" />
                  }
                </div>
                <div className="review-card-body">
                  <div className="review-card-price">{formatPriceCompact(p.price) ?? '—'}</div>
                  {address && <div className="review-card-address">{address}</div>}
                  <div className="review-card-stats">
                    {p.bedrooms != null && <span>{p.bedrooms} bd</span>}
                    {p.bathrooms != null && <span>{p.bathrooms} ba</span>}
                    {p.sqft != null && <span>{p.sqft.toLocaleString()} sqft</span>}
                  </div>
                </div>
                <div className="review-card-actions">
                  {STATUS_OPTIONS.filter(s => s !== 'Pending').map(s => (
                    <button
                      key={s}
                      className="review-action-btn"
                      style={{ '--action-color': STATUS_COLORS[s] } as React.CSSProperties}
                      onClick={() => assign(p.id, s)}
                      disabled={assigning === p.id}
                    >
                      {s}
                    </button>
                  ))}
                  <button
                    className="review-view-btn"
                    onClick={() => navigate(`/property/${p.id}`)}
                  >
                    View →
                  </button>
                </div>
              </div>
            )
          })}
        </div>
      )}
    </section>
  )
}
