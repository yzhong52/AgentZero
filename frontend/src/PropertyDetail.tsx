import { useEffect, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import type { Property, ImageEntry } from './App'
import { STATUS_OPTIONS, STATUS_COLORS } from './App'

type HistoryEntry = {
    id: number
    listing_id: number
    field_name: string
    old_value: string | null
    new_value: string | null
    changed_at: string
}

function boolLabel(v: boolean | null): string {
    return v === null ? '—' : v ? 'Yes' : 'No'
}

function moneyLabel(v: number | null): string {
    return v === null ? '—' : `$${v.toLocaleString()}`
}

function numLabel(v: number | null, suffix = ''): string {
    return v === null ? '—' : `${v}${suffix}`
}

function formatPrice(price: number | null, currency: string | null) {
    if (price == null) return null
    return new Intl.NumberFormat('en-CA', {
        style: 'currency',
        currency: currency ?? 'CAD',
        maximumFractionDigits: 0,
    }).format(price)
}

function formatImgDate(dateStr: string) {
    if (!dateStr) return ''
    return new Date(dateStr).toLocaleDateString('en-CA', {
        month: 'short',
        day: 'numeric',
        year: 'numeric',
    })
}

function ImageTile({
    img,
    alt,
    className,
    wrapperClass,
    onDelete,
}: {
    img: ImageEntry
    alt: string
    className: string
    wrapperClass: string
    onDelete: (id: number) => void
}) {
    return (
        <div className={wrapperClass}>
            <img src={img.url} alt={alt} className={className} />
            <span className="image-created-at">{formatImgDate(img.created_at)}</span>
            <button
                className="image-delete-btn"
                onClick={(e) => { e.stopPropagation(); onDelete(img.id) }}
                title="Delete image"
                aria-label="Delete image"
            >
                ×
            </button>
        </div>
    )
}

export function PropertyDetail() {
    const { id } = useParams<{ id: string }>()
    const navigate = useNavigate()
    const [property, setProperty] = useState<Property | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [refreshing, setRefreshing] = useState(false)
    const [refreshMsg, setRefreshMsg] = useState<string | null>(null)
    const [notes, setNotes] = useState<string>('')
    const [notesSaving, setNotesSaving] = useState(false)
    const [deleting, setDeleting] = useState(false)
    const [history, setHistory] = useState<HistoryEntry[]>([])
    const [nickname, setNickname] = useState<string>('')

    useEffect(() => {
        async function fetchProperty() {
            try {
                setLoading(true)
                const resp = await fetch('/api/listings')
                if (!resp.ok) throw new Error('Failed to fetch listings')
                const listings: Property[] = await resp.json()
                const found = listings.find((p) => p.id === parseInt(id!))
                if (!found) throw new Error('Property not found')
                setProperty(found)
                setNotes(found.notes ?? '')
                setNickname(found.nickname ?? '')

                const histResp = await fetch(`/api/listings/${found.id}/history`)
                if (histResp.ok) setHistory(await histResp.json())
            } catch (err: any) {
                setError(err?.message || String(err))
            } finally {
                setLoading(false)
            }
        }

        fetchProperty()
    }, [id])

    async function handleRefresh() {
        if (!property) return
        setError(null)
        setRefreshMsg(null)
        setRefreshing(true)
        try {
            const resp = await fetch(`/api/listings/${property.id}`, {
                method: 'PUT',
            })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty(updated)
            setRefreshMsg('Property updated successfully')
            setTimeout(() => setRefreshMsg(null), 3000)
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setRefreshing(false)
        }
    }

    async function handleNotesSave() {
        if (!property) return
        setNotesSaving(true)
        try {
            const resp = await fetch(`/api/listings/${property.id}/notes`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ notes: notes || null }),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setNotesSaving(false)
        }
    }

    async function handleNicknameSave() {
        if (!property) return
        setProperty({ ...property, nickname: nickname || null })
        try {
            const resp = await fetch(`/api/listings/${property.id}/nickname`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ nickname: nickname || null }),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

    async function handleStatusChange(newStatus: string) {
        if (!property) return
        setProperty({ ...property, status: newStatus || null })
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({
                    parking_garage: property.parking_garage,
                    parking_covered: property.parking_covered,
                    parking_open: property.parking_open,
                    land_sqft: property.land_sqft,
                    property_tax: property.property_tax,
                    skytrain_station: property.skytrain_station,
                    skytrain_walk_min: property.skytrain_walk_min,
                    radiant_floor_heating: property.radiant_floor_heating,
                    ac: property.ac,
                    mortgage_monthly: property.mortgage_monthly,
                    hoa_monthly: property.hoa_monthly,
                    monthly_total: property.monthly_total,
                    has_rental_suite: property.has_rental_suite,
                    rental_income: property.rental_income,
                    status: newStatus || null,
                }),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

    async function handleDelete() {
        if (!property) return
        if (!window.confirm(`Delete "${property.title}"? This cannot be undone.`)) return
        setDeleting(true)
        try {
            const resp = await fetch(`/api/listings/${property.id}`, { method: 'DELETE' })
            if (!resp.ok) throw new Error(await resp.text())
            navigate('/')
        } catch (err: any) {
            setError(err?.message || String(err))
            setDeleting(false)
        }
    }

    async function handleDeleteImage(imageId: number) {
        if (!property) return
        try {
            const resp = await fetch(`/api/listings/${property.id}/images/${imageId}`, {
                method: 'DELETE',
            })
            if (!resp.ok) throw new Error(await resp.text())
            setProperty({ ...property, images: property.images.filter((img) => img.id !== imageId) })
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

    if (loading) return <div className="loading">Loading...</div>
    if (error) return <div className="error-msg">{error}</div>
    if (!property) return <div className="error-msg">Property not found</div>

    const address = [
        property.street_address,
        property.city,
        property.region,
        property.postal_code,
    ]
        .filter(Boolean)
        .join(', ')

    return (
        <div className="property-detail">
            <div className="detail-nav">
                <button className="back-btn" onClick={() => navigate('/')}>
                    ← Back
                </button>
                <button
                    className="refresh-btn"
                    onClick={handleRefresh}
                    disabled={refreshing}
                    title="Refresh property data from source"
                >
                    {refreshing ? '⟳ Refreshing…' : '⟳ Refresh'}
                </button>
                <button
                    className="delete-btn"
                    onClick={handleDelete}
                    disabled={deleting}
                    title="Delete this listing"
                >
                    {deleting ? 'Deleting…' : 'Delete'}
                </button>
            </div>

            {error && <div className="message error">{error}</div>}
            {refreshMsg && <div className="message success">{refreshMsg}</div>}

            <div className="detail-images">
                {property.images.length > 0 ? (
                    <div className="image-carousel">
                        <ImageTile
                            img={property.images[0]}
                            alt={property.title}
                            className="main-image"
                            wrapperClass="image-wrapper main-image-wrapper"
                            onDelete={handleDeleteImage}
                        />
                        {property.images.length > 1 && (
                            <div className="image-thumbnails">
                                {property.images.map((img) => (
                                    <ImageTile
                                        key={img.id}
                                        img={img}
                                        alt={property.title}
                                        className="thumbnail"
                                        wrapperClass="image-wrapper thumbnail-wrapper"
                                        onDelete={handleDeleteImage}
                                    />
                                ))}
                            </div>
                        )}
                    </div>
                ) : (
                    <div className="no-image">No images available</div>
                )}
            </div>

            <div className="detail-body">
                <div className="detail-content">
                    <div className="detail-header">
                        <input
                            className="nickname-input"
                            value={nickname}
                            onChange={(e) => setNickname(e.target.value)}
                            onBlur={handleNicknameSave}
                            placeholder={property.title}
                            aria-label="Property nickname"
                        />
                        {nickname && <div className="detail-subtitle">{property.title}</div>}
                        <div className="detail-price">{formatPrice(property.price, property.price_currency)}</div>
                    </div>

                    {address && <div className="detail-address">{address}</div>}

                    {property.description && (
                        <div className="detail-description">
                            <h3>Description</h3>
                            <p>{property.description}</p>
                        </div>
                    )}

                    <div className="tracked-details">
                        <h3>Details</h3>

                        <div className="tracked-group">
                            <h4>Property</h4>
                            <div className="tracked-fields">
                                {property.bedrooms != null && <div className="tracked-field"><label>Bedrooms</label><span className="tracked-value">{property.bedrooms}</span></div>}
                                {property.bathrooms != null && <div className="tracked-field"><label>Bathrooms</label><span className="tracked-value">{property.bathrooms}</span></div>}
                                {property.sqft != null && <div className="tracked-field"><label>Square feet</label><span className="tracked-value">{property.sqft.toLocaleString()}</span></div>}
                                {property.year_built != null && <div className="tracked-field"><label>Year built</label><span className="tracked-value">{property.year_built}</span></div>}
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Parking</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Garage (indoor)</label><span className="tracked-value">{numLabel(property.parking_garage)}</span></div>
                                <div className="tracked-field"><label>Covered outdoor</label><span className="tracked-value">{numLabel(property.parking_covered)}</span></div>
                                <div className="tracked-field"><label>Open outdoor</label><span className="tracked-value">{numLabel(property.parking_open)}</span></div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Land</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Land size (sqft)</label><span className="tracked-value">{numLabel(property.land_sqft, ' sqft')}</span></div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Transit</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Closest Skytrain station</label><span className="tracked-value">{property.skytrain_station ?? '—'}</span></div>
                                <div className="tracked-field"><label>Walk time (min)</label><span className="tracked-value">{numLabel(property.skytrain_walk_min, ' min')}</span></div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Features</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Radiant floor heating</label><span className="tracked-value">{boolLabel(property.radiant_floor_heating)}</span></div>
                                <div className="tracked-field"><label>Air conditioning</label><span className="tracked-value">{boolLabel(property.ac)}</span></div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Financials</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Property tax (annual)</label><span className="tracked-value">{moneyLabel(property.property_tax)}</span></div>
                                <div className="tracked-field"><label>Mortgage (monthly)</label><span className="tracked-value">{moneyLabel(property.mortgage_monthly)}</span></div>
                                <div className="tracked-field"><label>HOA / Strata (monthly)</label><span className="tracked-value">{moneyLabel(property.hoa_monthly)}</span></div>
                                <div className="tracked-field"><label>Monthly total</label><span className="tracked-value">{moneyLabel(property.monthly_total)}</span></div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Rental</h4>
                            <div className="tracked-fields">
                                <div className="tracked-field"><label>Has rental suite</label><span className="tracked-value">{boolLabel(property.has_rental_suite)}</span></div>
                                {property.has_rental_suite !== false && (
                                    <div className="tracked-field"><label>Rental income (monthly)</label><span className="tracked-value">{moneyLabel(property.rental_income)}</span></div>
                                )}
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Nearby Schools <span className="school-source-note">(Fraser Institute rating /10)</span></h4>
                            <div className="tracked-fields">
                                <div className="tracked-field">
                                    <label>Elementary</label>
                                    <span className="tracked-value school-entry">
                                        {property.school_elementary ?? '—'}
                                        {property.school_elementary_rating != null && (
                                            <span className="school-rating">{property.school_elementary_rating.toFixed(1)}</span>
                                        )}
                                    </span>
                                </div>
                                <div className="tracked-field">
                                    <label>Middle</label>
                                    <span className="tracked-value school-entry">
                                        {property.school_middle ?? '—'}
                                        {property.school_middle_rating != null && (
                                            <span className="school-rating">{property.school_middle_rating.toFixed(1)}</span>
                                        )}
                                    </span>
                                </div>
                                <div className="tracked-field">
                                    <label>Secondary</label>
                                    <span className="tracked-value school-entry">
                                        {property.school_secondary ?? '—'}
                                        {property.school_secondary_rating != null && (
                                            <span className="school-rating">{property.school_secondary_rating.toFixed(1)}</span>
                                        )}
                                    </span>
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="detail-metadata">
                        <div className="meta-item">
                            <strong>URL:</strong>
                            <a href={property.url} target="_blank" rel="noreferrer">{property.url}</a>
                        </div>
                        <div className="meta-item">
                            <strong>Watched since:</strong> {new Date(property.created_at).toLocaleDateString('en-CA', { month: 'short', day: 'numeric', year: 'numeric' })}
                        </div>
                        <div className="meta-item">
                            <strong>Last refreshed:</strong> {property.updated_at ? new Date(property.updated_at).toLocaleDateString('en-CA', { month: 'short', day: 'numeric', year: 'numeric' }) : '—'}
                        </div>
                    </div>

                    {property.lat != null && property.lon != null && (
                        <div className="map-preview">
                            <iframe
                                title="Property location"
                                src={`https://maps.google.com/maps?q=${property.lat},${property.lon}&z=15&output=embed`}
                                loading="lazy"
                                referrerPolicy="no-referrer-when-downgrade"
                            />
                        </div>
                    )}
                </div>

                <div className="notes-panel">
                    <div className="status-picker">
                        <label className="status-picker-label">Status</label>
                        <div className="status-picker-buttons">
                            {STATUS_OPTIONS.map((s) => (
                                <button
                                    key={s}
                                    className={`status-option-btn${property.status === s ? ' active' : ''}`}
                                    style={property.status === s ? { background: STATUS_COLORS[s], color: '#fff', borderColor: STATUS_COLORS[s] } : {}}
                                    onClick={() => handleStatusChange(s)}
                                >
                                    {s}
                                </button>
                            ))}
                        </div>
                    </div>

                    <h3 className="notes-heading">My Notes</h3>
                    <textarea
                        className="notes-textarea"
                        value={notes}
                        onChange={(e) => setNotes(e.target.value)}
                        onBlur={handleNotesSave}
                        placeholder="Add personal notes about this property…"
                        disabled={notesSaving}
                    />
                    {notesSaving && <div className="notes-saving">Saving…</div>}

                    {history.length > 0 && (
                        <div className="history-panel">
                            <h3 className="notes-heading">Change History</h3>
                            <ul className="history-list">
                                {history.map((entry) => (
                                    <li key={entry.id} className="history-entry">
                                        <span className="history-field">{entry.field_name}</span>
                                        <span className="history-change">
                                            {entry.old_value ?? '—'} → {entry.new_value ?? '—'}
                                        </span>
                                        <span className="history-date">
                                            {new Date(entry.changed_at).toLocaleDateString('en-CA', {
                                                month: 'short', day: 'numeric', year: 'numeric',
                                            })}
                                        </span>
                                    </li>
                                ))}
                            </ul>
                        </div>
                    )}
                </div>
            </div>
        </div>
    )
}
