import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import type { Property } from './types'
import { displayStatus, STATUS_COLORS } from './constants'
import { formatPriceCompact } from './utils'
import './App.css'

const PAGE_CONFIG = {
  AgentPending: {
    title: 'Pending Review',
    desc: 'Properties waiting for the agent to analyze',
    emptyText: 'No listings waiting for agent review.',
  },
  AgentSkip: {
    title: 'Agent Skipped',
    desc: 'Listings the agent flagged as not a fit',
    emptyText: 'No listings skipped by the agent.',
  },
} as const

type QueueStatus = keyof typeof PAGE_CONFIG

export function AgentQueuePage({ status }: { status: QueueStatus }) {
  const navigate = useNavigate()
  const [listings, setListings] = useState<Property[]>([])
  const [loading, setLoading] = useState(true)
  const config = PAGE_CONFIG[status]

  useEffect(() => {
    async function load() {
      setLoading(true)
      try {
        const resp = await fetch(`/api/listings?status=${status}`)
        if (!resp.ok) return
        setListings(await resp.json())
      } finally {
        setLoading(false)
      }
    }
    load()
  }, [status])

  const isPending = status === 'AgentPending'

  return (
    <div className="agent-queue-page">
      <div className="agent-queue-header">
        <button className="back-btn" onClick={() => navigate('/')}>
          <svg width="7" height="12" viewBox="0 0 7 12" fill="none" aria-hidden="true">
            <path d="M6 1L1 6l5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" />
          </svg>
          Back
        </button>
        <h1 className="agent-queue-title">{config.title}</h1>
      </div>

      {loading ? (
        <div className="loading">Loading…</div>
      ) : (
        <section className="agent-queue-section">
          <div className="agent-queue-section-header">
            <div className="agent-queue-section-title-row">
              <span
                className="agent-queue-status-dot"
                style={{ background: STATUS_COLORS[status] ?? '#9ca3af' }}
              />
              <h2 className="agent-queue-section-title">{config.title}</h2>
              {listings.length > 0 && (
                <span className="agent-queue-count">{listings.length}</span>
              )}
            </div>
            <p className="agent-queue-section-desc">{config.desc}</p>
          </div>

          {listings.length === 0 ? (
            <p className="agent-queue-empty">{config.emptyText}</p>
          ) : (
            <ul className="agent-queue-list">
              {listings.map(p => (
                <li
                  key={p.id}
                  className={`agent-queue-item${isPending ? '' : ' agent-queue-item--skipped'}`}
                  onClick={() => navigate(`/property/${p.id}`)}
                >
                  <div className="agent-queue-item-thumb">
                    {p.images[0]?.url ? (
                      <img src={p.images[0].url} alt={p.title ?? ''} />
                    ) : isPending ? (
                      <div className="agent-queue-item-thumb--shimmer" />
                    ) : (
                      <div className="agent-queue-item-thumb-empty" />
                    )}
                  </div>
                  <div className="agent-queue-item-body">
                    <div className="agent-queue-item-top">
                      <span className="agent-queue-item-price">
                        {formatPriceCompact(p.price) ?? '—'}
                      </span>
                      {p.source_status && (
                        <span className="inbox-source-status">{p.source_status}</span>
                      )}
                      <span
                        className="agent-queue-item-badge"
                        style={{ '--badge-color': STATUS_COLORS[status] ?? '#9ca3af' } as React.CSSProperties}
                      >
                        {displayStatus(status)}
                      </span>
                    </div>
                    {p.street_address && (
                      <div className="agent-queue-item-address">{p.street_address}</div>
                    )}
                    {p.agent_review_comment ? (
                      <div className="agent-queue-item-comment">
                        <span className="agent-queue-item-comment-label">Agent</span>
                        {p.agent_review_comment}
                      </div>
                    ) : isPending ? (
                      <div className="agent-queue-item-analyzing">
                        Analyzing
                        <span className="agent-queue-item-analyzing-dots" aria-hidden="true">
                          <span /><span /><span />
                        </span>
                      </div>
                    ) : null}
                  </div>
                </li>
              ))}
            </ul>
          )}
        </section>
      )}
    </div>
  )
}
