import { useEffect, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import type { Property } from './App'
import { STATUS_OPTIONS, STATUS_COLORS } from './App'

type HistoryEntry = {
    id: number
    listing_id: number
    field_name: string
    old_value: string | null
    new_value: string | null
    changed_at: string
}

// ── Helpers ───────────────────────────────────────────────────────────────────

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
        month: 'short', day: 'numeric', year: 'numeric',
    })
}

/** Compute standard monthly mortgage payment. */
function calcMortgage(price: number | null, downPct: number, rate: number, years: number): number | null {
    if (!price) return null
    const loan = price * (1 - downPct)
    if (loan <= 0) return 0
    const n = years * 12
    if (rate === 0) return Math.round(loan / n)
    const r = rate / 12
    return Math.round(loan * r * Math.pow(1 + r, n) / (Math.pow(1 + r, n) - 1))
}


// ── Edit helpers ──────────────────────────────────────────────────────────────

function TextInput({ label, value, onChange }: {
    label: string; value: string | null; onChange: (v: string | null) => void
}) {
    return (
        <div className="tracked-field">
            <label>{label}</label>
            <input
                className="edit-input"
                value={value ?? ''}
                onChange={e => onChange(e.target.value || null)}
            />
        </div>
    )
}

function NumInput({ label, value, onChange, suffix }: {
    label: string; value: number | null; onChange: (v: number | null) => void; suffix?: string
}) {
    return (
        <div className="tracked-field">
            <label>{label}{suffix ? ` (${suffix})` : ''}</label>
            <input
                className="edit-input"
                type="number"
                value={value ?? ''}
                onChange={e => onChange(e.target.value ? Number(e.target.value) : null)}
            />
        </div>
    )
}

function BoolSelect({ label, value, onChange }: {
    label: string; value: boolean | null; onChange: (v: boolean | null) => void
}) {
    return (
        <div className="tracked-field">
            <label>{label}</label>
            <select
                className="edit-input"
                value={value === null ? '' : value ? 'true' : 'false'}
                onChange={e => onChange(e.target.value === '' ? null : e.target.value === 'true')}
            >
                <option value="">—</option>
                <option value="true">Yes</option>
                <option value="false">No</option>
            </select>
        </div>
    )
}

// ── Diff modal ────────────────────────────────────────────────────────────────

type DiffEntry = { field: string; old: string; fresh: string }

function RefreshDiffModal({
    diffs,
    onApply,
    onCancel,
    applying,
}: {
    diffs: DiffEntry[]
    onApply: () => void
    onCancel: () => void
    applying: boolean
}) {
    return (
        <div className="modal-overlay">
            <div className="modal">
                <h3>Changes found from source</h3>
                {diffs.length === 0 ? (
                    <p className="no-changes">No changes detected — listing is up to date.</p>
                ) : (
                    <table className="diff-table">
                        <thead>
                            <tr><th>Field</th><th>Stored</th><th>Fresh</th></tr>
                        </thead>
                        <tbody>
                            {diffs.map(d => (
                                <tr key={d.field}>
                                    <td className="diff-field">{d.field}</td>
                                    <td className="diff-old">{d.old}</td>
                                    <td className="diff-new">{d.fresh}</td>
                                </tr>
                            ))}
                        </tbody>
                    </table>
                )}
                <div className="modal-actions">
                    {diffs.length > 0 && (
                        <button className="save-btn" onClick={onApply} disabled={applying}>
                            {applying ? 'Applying…' : 'Apply changes'}
                        </button>
                    )}
                    <button className="cancel-btn" onClick={onCancel} disabled={applying}>
                        {diffs.length === 0 ? 'Close' : 'Cancel'}
                    </button>
                </div>
            </div>
        </div>
    )
}

// ── Compare helper ────────────────────────────────────────────────────────────

function str(v: unknown): string {
    if (v === null || v === undefined) return '—'
    if (typeof v === 'boolean') return v ? 'Yes' : 'No'
    return String(v)
}

const DIFF_FIELDS: { key: keyof Property; label: string }[] = [
    { key: 'price',              label: 'Price' },
    { key: 'street_address',     label: 'Address' },
    { key: 'city',               label: 'City' },
    { key: 'region',             label: 'Region' },
    { key: 'postal_code',        label: 'Postal code' },
    { key: 'bedrooms',           label: 'Bedrooms' },
    { key: 'bathrooms',          label: 'Bathrooms' },
    { key: 'sqft',               label: 'Sqft' },
    { key: 'year_built',         label: 'Year built' },
    { key: 'land_sqft',          label: 'Land sqft' },
    { key: 'parking_garage',     label: 'Garage' },
    { key: 'ac',                 label: 'Air conditioning' },
    { key: 'radiant_floor_heating', label: 'Radiant heating' },
    { key: 'school_elementary',  label: 'Elementary school' },
    { key: 'school_middle',      label: 'Middle school' },
    { key: 'school_secondary',   label: 'Secondary school' },
]

function buildDiff(stored: Property, fresh: Property): DiffEntry[] {
    return DIFF_FIELDS
        .filter(f => str(stored[f.key]) !== str(fresh[f.key]))
        .map(f => ({ field: f.label, old: str(stored[f.key]), fresh: str(fresh[f.key]) }))
}

// ── Build details payload (all editable fields) ───────────────────────────────

function toUserDetails(p: Property) {
    return {
        redfin_url:             p.redfin_url,
        realtor_url:            p.realtor_url,
        rew_url:                p.rew_url,
        price:                  p.price,
        price_currency:         p.price_currency,
        street_address:         p.street_address,
        city:                   p.city,
        region:                 p.region,
        postal_code:            p.postal_code,
        bedrooms:               p.bedrooms,
        bathrooms:              p.bathrooms,
        sqft:                   p.sqft,
        year_built:             p.year_built,
        parking_garage:         p.parking_garage,
        parking_covered:        p.parking_covered,
        parking_open:           p.parking_open,
        land_sqft:              p.land_sqft,
        property_tax:           p.property_tax,
        skytrain_station:       p.skytrain_station,
        skytrain_walk_min:      p.skytrain_walk_min,
        radiant_floor_heating:  p.radiant_floor_heating,
        ac:                     p.ac,
        down_payment_pct:       p.down_payment_pct,
        mortgage_interest_rate: p.mortgage_interest_rate,
        amortization_years:     p.amortization_years,
        mortgage_monthly:       p.mortgage_monthly,
        hoa_monthly:            p.hoa_monthly,
        has_rental_suite:       p.has_rental_suite,
        rental_income:          p.rental_income,
        status:                 p.status,
        school_elementary:              p.school_elementary,
        school_elementary_rating:       p.school_elementary_rating,
        school_middle:                  p.school_middle,
        school_middle_rating:           p.school_middle_rating,
        school_secondary:               p.school_secondary,
        school_secondary_rating:        p.school_secondary_rating,
    }
}

// ── Main component ────────────────────────────────────────────────────────────

export function PropertyDetail() {
    const { id } = useParams<{ id: string }>()
    const navigate = useNavigate()

    const [property, setProperty] = useState<Property | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)
    const [refreshMsg, setRefreshMsg] = useState<string | null>(null)

    // Notes
    const [notes, setNotes] = useState<string>('')
    const [notesSaving, setNotesSaving] = useState(false)

    // Nickname (inline header)
    const [nickname, setNickname] = useState<string>('')

    // Edit mode
    const [editMode, setEditMode] = useState(false)
    const [draft, setDraft] = useState<Property | null>(null)
    const [saving, setSaving] = useState(false)

    // Refresh preview
    const [previewing, setPreviewing] = useState(false)
    const [applying, setApplying] = useState(false)
    const [diffModal, setDiffModal] = useState<DiffEntry[] | null>(null)

    // Delete
    const [deleting, setDeleting] = useState(false)

    // Lightbox
    const [lightboxOpen, setLightboxOpen] = useState(false)
    const [activeIdx, setActiveIdx] = useState(0)
    const thumbsRef = useRef<HTMLDivElement>(null)
    const scrollRef = useRef<HTMLDivElement>(null)

    // History
    const [history, setHistory] = useState<HistoryEntry[]>([])

    // ── Data loading ──────────────────────────────────────────────────────────

    async function loadProperty() {
        try {
            setLoading(true)
            const resp = await fetch(`/api/listings/${id}`)
            if (!resp.ok) throw new Error('Property not found')
            const p: Property = await resp.json()
            setProperty(p)
            setNotes(p.notes ?? '')
            setNickname(p.nickname ?? '')

            const histResp = await fetch(`/api/listings/${id}/history`)
            if (histResp.ok) setHistory(await histResp.json())
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setLoading(false)
        }
    }

    useEffect(() => { loadProperty() }, [id])

    // ── Edit mode ─────────────────────────────────────────────────────────────

    function enterEdit() {
        setDraft(property ? { ...property } : null)
        setEditMode(true)
    }

    function cancelEdit() {
        setEditMode(false)
        setDraft(null)
    }

    function setDraftField<K extends keyof Property>(key: K, val: Property[K]) {
        setDraft(d => d ? { ...d, [key]: val } : d)
    }

    /** Recalculate mortgage_monthly in draft whenever price or params change. */
    function recalcMortgage(d: Property): Property {
        const monthly = calcMortgage(
            d.price,
            d.down_payment_pct ?? 0.20,
            d.mortgage_interest_rate ?? 0.05,
            d.amortization_years ?? 25,
        )
        return { ...d, mortgage_monthly: monthly }
    }

    async function handleSaveEdits() {
        if (!draft || !property) return
        setSaving(true)
        setError(null)
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(toUserDetails(draft)),
            })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty({ ...updated, images: property.images })
            setEditMode(false)
            setDraft(null)
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setSaving(false)
        }
    }

    // ── Refresh with preview diff ─────────────────────────────────────────────

    async function handleRefreshPreview() {
        if (!property) return
        setError(null)
        setRefreshMsg(null)
        setPreviewing(true)
        try {
            const resp = await fetch(`/api/listings/${property.id}/preview`)
            if (!resp.ok) throw new Error(await resp.text())
            const fresh: Property = await resp.json()
            setDiffModal(buildDiff(property, fresh))
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setPreviewing(false)
        }
    }

    async function applyRefresh() {
        if (!property) return
        setApplying(true)
        try {
            const resp = await fetch(`/api/listings/${property.id}`, { method: 'PUT' })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty(updated)
            setDiffModal(null)
            setRefreshMsg('Property updated successfully')
            setTimeout(() => setRefreshMsg(null), 3000)
            // Reload history
            const histResp = await fetch(`/api/listings/${property.id}/history`)
            if (histResp.ok) setHistory(await histResp.json())
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setApplying(false)
        }
    }

    // ── Notes ─────────────────────────────────────────────────────────────────

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

    // ── Nickname ──────────────────────────────────────────────────────────────

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

    // ── Status ────────────────────────────────────────────────────────────────

    async function handleStatusChange(newStatus: string) {
        if (!property) return
        const updated = { ...property, status: newStatus || null }
        setProperty(updated)
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(toUserDetails(updated)),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

    // ── Delete ────────────────────────────────────────────────────────────────

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

    // ── Image delete ──────────────────────────────────────────────────────────

    async function handleDeleteImage(imageId: number) {
        if (!property) return
        try {
            const resp = await fetch(`/api/listings/${property.id}/images/${imageId}`, { method: 'DELETE' })
            if (!resp.ok) throw new Error(await resp.text())
            setProperty({ ...property, images: property.images.filter(img => img.id !== imageId) })
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

// ── Render ────────────────────────────────────────────────────────────────

    if (loading) return <div className="loading">Loading...</div>
    if (error && !property) return <div className="error-msg">{error}</div>
    if (!property) return <div className="error-msg">Property not found</div>

    const address = [property.street_address, property.city, property.region, property.postal_code]
        .filter(Boolean).join(', ')

    // What to render: draft in edit mode, property otherwise
    const p = editMode && draft ? draft : property

    // Helper to wrap a field: in edit mode shows input, else shows static value
    function Field({ label, viewVal, editEl }: {
        label: string; viewVal: string; editEl?: React.ReactNode
    }) {
        if (editMode && editEl) return <>{editEl}</>
        return (
            <div className="tracked-field">
                <label>{label}</label>
                <span className="tracked-value">{viewVal}</span>
            </div>
        )
    }

    return (
        <div className="property-detail">
            {diffModal !== null && (
                <RefreshDiffModal
                    diffs={diffModal}
                    onApply={applyRefresh}
                    onCancel={() => setDiffModal(null)}
                    applying={applying}
                />
            )}

            <div className="detail-nav">
                <button className="back-btn" onClick={() => navigate('/')}>← Back</button>
                <button
                    className="refresh-btn"
                    onClick={handleRefreshPreview}
                    disabled={previewing || editMode}
                    title="Preview changes from source"
                >
                    {previewing ? '⟳ Checking…' : '⟳ Refresh'}
                </button>
                {!editMode ? (
                    <button className="edit-btn" onClick={enterEdit}>Edit</button>
                ) : (
                    <>
                        <button className="save-btn" onClick={handleSaveEdits} disabled={saving}>
                            {saving ? 'Saving…' : 'Save'}
                        </button>
                        <button className="cancel-btn" onClick={cancelEdit} disabled={saving}>Cancel</button>
                    </>
                )}
                <button
                    className="delete-btn"
                    onClick={handleDelete}
                    disabled={deleting || editMode}
                    title="Delete this listing"
                >
                    {deleting ? 'Deleting…' : 'Delete'}
                </button>
            </div>

            {error && <div className="message error">{error}</div>}
            {refreshMsg && <div className="message success">{refreshMsg}</div>}

            {lightboxOpen && property.images.length > 0 && (
                <div className="modal-overlay" onClick={() => setLightboxOpen(false)}>
                    <div className="lightbox-panel" onClick={e => e.stopPropagation()}>
                        <div className="lightbox-header">
                            <button className="lightbox-close" onClick={() => setLightboxOpen(false)}>✕</button>
                            <span className="lightbox-title">{property.nickname || property.title}</span>
                            <span className="lightbox-count">{property.images.length} photos</span>
                        </div>
                        <div className="lightbox-body">
                            <div className="lightbox-thumbs" ref={thumbsRef}>
                                {property.images.map((img, i) => (
                                    <img
                                        key={img.id}
                                        src={img.url}
                                        alt={`${i + 1}`}
                                        className={`lightbox-thumb${activeIdx === i ? ' active' : ''}`}
                                        onClick={() => {
                                            setActiveIdx(i)
                                            scrollRef.current
                                                ?.querySelectorAll<HTMLElement>('.lightbox-item')
                                                [i]?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
                                        }}
                                    />
                                ))}
                            </div>
                            <div
                                className="lightbox-scroll"
                                ref={scrollRef}
                                onScroll={() => {
                                    const container = scrollRef.current
                                    if (!container) return
                                    const items = container.querySelectorAll<HTMLElement>('.lightbox-item')
                                    let closest = 0
                                    let minDist = Infinity
                                    items.forEach((el, i) => {
                                        const dist = Math.abs(el.getBoundingClientRect().top - container.getBoundingClientRect().top)
                                        if (dist < minDist) { minDist = dist; closest = i }
                                    })
                                    if (closest !== activeIdx) {
                                        setActiveIdx(closest)
                                        const thumb = thumbsRef.current?.querySelectorAll<HTMLElement>('.lightbox-thumb')[closest]
                                        thumb?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
                                    }
                                }}
                            >
                                {property.images.map((img, i) => (
                                    <div key={img.id} className="lightbox-item">
                                        <img src={img.url} alt={`${property.title} — ${i + 1}`} className="lightbox-img" />
                                        <span className="lightbox-caption">{i + 1} / {property.images.length}</span>
                                        <span className="lightbox-date">{formatImgDate(img.created_at)}</span>
                                        <button
                                            className="lightbox-delete-btn"
                                            title="Delete image"
                                            onClick={e => { e.stopPropagation(); handleDeleteImage(img.id) }}
                                        >✕</button>
                                    </div>
                                ))}
                            </div>
                        </div>
                    </div>
                </div>
            )}

            <div className="detail-images">
                {property.images.length > 0 ? (
                    <div className="image-grid" onClick={() => setLightboxOpen(true)}>
                        {property.images.slice(0, 3).map((img, i) => (
                            <div
                                key={img.id}
                                className={`image-grid-cell${i === 0 ? ' image-grid-main' : ''}`}
                            >
                                <img src={img.url} alt={property.title} className="image-grid-img" />
                                {i === 2 && property.images.length > 3 && (
                                    <div className="image-grid-more">+{property.images.length - 3}</div>
                                )}
                            </div>
                        ))}
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
                            onChange={e => setNickname(e.target.value)}
                            onBlur={handleNicknameSave}
                            placeholder={property.title}
                            aria-label="Property nickname"
                        />
                        {nickname && <div className="detail-subtitle">{property.title}</div>}
                        <div className="detail-price">{formatPrice(p.price, p.price_currency)}</div>
                    </div>

                    {!editMode && address && <div className="detail-address">{address}</div>}

                    {property.description && (
                        <div className="detail-description">
                            <h3>Description</h3>
                            <p>{property.description}</p>
                        </div>
                    )}

                    <div className="tracked-details">
                        <div className="tracked-details-header">
                            <h3>Details</h3>
                            {editMode && <span className="edit-mode-badge">Editing</span>}
                        </div>

                        <div className="tracked-group">
                            <h4>Location</h4>
                            <div className="tracked-fields">
                                <Field label="Street address" viewVal={p.street_address ?? '—'}
                                    editEl={<TextInput label="Street address" value={draft?.street_address ?? null} onChange={v => setDraftField('street_address', v)} />} />
                                <Field label="City" viewVal={p.city ?? '—'}
                                    editEl={<TextInput label="City" value={draft?.city ?? null} onChange={v => setDraftField('city', v)} />} />
                                <Field label="Region / Province" viewVal={p.region ?? '—'}
                                    editEl={<TextInput label="Region / Province" value={draft?.region ?? null} onChange={v => setDraftField('region', v)} />} />
                                <Field label="Postal code" viewVal={p.postal_code ?? '—'}
                                    editEl={<TextInput label="Postal code" value={draft?.postal_code ?? null} onChange={v => setDraftField('postal_code', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Property</h4>
                            <div className="tracked-fields">
                                {(p.bedrooms != null || editMode) && (
                                    <Field label="Bedrooms" viewVal={numLabel(p.bedrooms)}
                                        editEl={<NumInput label="Bedrooms" value={draft?.bedrooms ?? null} onChange={v => setDraftField('bedrooms', v)} />} />
                                )}
                                {(p.bathrooms != null || editMode) && (
                                    <Field label="Bathrooms" viewVal={numLabel(p.bathrooms)}
                                        editEl={<NumInput label="Bathrooms" value={draft?.bathrooms ?? null} onChange={v => setDraftField('bathrooms', v)} />} />
                                )}
                                {(p.sqft != null || editMode) && (
                                    <Field label="Square feet" viewVal={p.sqft != null ? p.sqft.toLocaleString() : '—'}
                                        editEl={<NumInput label="Square feet" value={draft?.sqft ?? null} onChange={v => setDraftField('sqft', v)} />} />
                                )}
                                {(p.year_built != null || editMode) && (
                                    <Field label="Year built" viewVal={numLabel(p.year_built)}
                                        editEl={<NumInput label="Year built" value={draft?.year_built ?? null} onChange={v => setDraftField('year_built', v)} />} />
                                )}
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Parking</h4>
                            <div className="tracked-fields">
                                <Field label="Garage (indoor)" viewVal={numLabel(p.parking_garage)}
                                    editEl={<NumInput label="Garage (indoor)" value={draft?.parking_garage ?? null} onChange={v => setDraftField('parking_garage', v)} />} />
                                <Field label="Covered outdoor" viewVal={numLabel(p.parking_covered)}
                                    editEl={<NumInput label="Covered outdoor" value={draft?.parking_covered ?? null} onChange={v => setDraftField('parking_covered', v)} />} />
                                <Field label="Open outdoor" viewVal={numLabel(p.parking_open)}
                                    editEl={<NumInput label="Open outdoor" value={draft?.parking_open ?? null} onChange={v => setDraftField('parking_open', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Land</h4>
                            <div className="tracked-fields">
                                <Field label="Land size (sqft)" viewVal={numLabel(p.land_sqft, ' sqft')}
                                    editEl={<NumInput label="Land size (sqft)" value={draft?.land_sqft ?? null} onChange={v => setDraftField('land_sqft', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Transit</h4>
                            <div className="tracked-fields">
                                <Field label="Closest Skytrain station" viewVal={p.skytrain_station ?? '—'}
                                    editEl={<TextInput label="Closest Skytrain station" value={draft?.skytrain_station ?? null} onChange={v => setDraftField('skytrain_station', v)} />} />
                                <Field label="Walk time (min)" viewVal={numLabel(p.skytrain_walk_min, ' min')}
                                    editEl={<NumInput label="Walk time (min)" value={draft?.skytrain_walk_min ?? null} onChange={v => setDraftField('skytrain_walk_min', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Features</h4>
                            <div className="tracked-fields">
                                <Field label="Radiant floor heating" viewVal={boolLabel(p.radiant_floor_heating)}
                                    editEl={<BoolSelect label="Radiant floor heating" value={draft?.radiant_floor_heating ?? null} onChange={v => setDraftField('radiant_floor_heating', v)} />} />
                                <Field label="Air conditioning" viewVal={boolLabel(p.ac)}
                                    editEl={<BoolSelect label="Air conditioning" value={draft?.ac ?? null} onChange={v => setDraftField('ac', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Financials</h4>
                            <div className="tracked-fields">
                                <Field label="Price" viewVal={formatPrice(p.price, p.price_currency) ?? '—'}
                                    editEl={
                                        <div className="tracked-field">
                                            <label>Price</label>
                                            <input
                                                className="edit-input"
                                                type="number"
                                                value={draft?.price ?? ''}
                                                onChange={e => {
                                                    const updated = recalcMortgage({ ...draft!, price: e.target.value ? Number(e.target.value) : null })
                                                    setDraft(updated)
                                                }}
                                                placeholder="Price"
                                            />
                                        </div>
                                    } />
                                <Field label="Property tax (annual)" viewVal={moneyLabel(p.property_tax)}
                                    editEl={<NumInput label="Property tax (annual)" value={draft?.property_tax ?? null} onChange={v => setDraftField('property_tax', v)} />} />
                                <Field label="HOA / Strata (monthly)" viewVal={moneyLabel(p.hoa_monthly)}
                                    editEl={<NumInput label="HOA / Strata (monthly)" value={draft?.hoa_monthly ?? null} onChange={v => setDraftField('hoa_monthly', v)} />} />

                                {/* Mortgage params */}
                                <Field label="Down payment %" viewVal={p.down_payment_pct != null ? `${(p.down_payment_pct * 100).toFixed(0)}%` : '—'}
                                    editEl={
                                        <div className="tracked-field">
                                            <label>Down payment %</label>
                                            <input
                                                className="edit-input"
                                                type="number"
                                                min={0} max={100} step={1}
                                                value={draft?.down_payment_pct != null ? (draft.down_payment_pct * 100).toFixed(0) : ''}
                                                onChange={e => {
                                                    const pct = e.target.value ? Number(e.target.value) / 100 : null
                                                    const updated = recalcMortgage({ ...draft!, down_payment_pct: pct })
                                                    setDraft(updated)
                                                }}
                                            />
                                        </div>
                                    } />
                                <Field label="Mortgage rate %" viewVal={p.mortgage_interest_rate != null ? `${(p.mortgage_interest_rate * 100).toFixed(2)}%` : '—'}
                                    editEl={
                                        <div className="tracked-field">
                                            <label>Mortgage rate %</label>
                                            <input
                                                className="edit-input"
                                                type="number"
                                                min={0} max={30} step={0.01}
                                                value={draft?.mortgage_interest_rate != null ? (draft.mortgage_interest_rate * 100).toFixed(2) : ''}
                                                onChange={e => {
                                                    const rate = e.target.value ? Number(e.target.value) / 100 : null
                                                    const updated = recalcMortgage({ ...draft!, mortgage_interest_rate: rate })
                                                    setDraft(updated)
                                                }}
                                            />
                                        </div>
                                    } />
                                <Field label="Amortization (years)" viewVal={numLabel(p.amortization_years, ' yr')}
                                    editEl={
                                        <div className="tracked-field">
                                            <label>Amortization (years)</label>
                                            <input
                                                className="edit-input"
                                                type="number"
                                                min={1} max={40} step={1}
                                                value={draft?.amortization_years ?? ''}
                                                onChange={e => {
                                                    const yrs = e.target.value ? Number(e.target.value) : null
                                                    const updated = recalcMortgage({ ...draft!, amortization_years: yrs })
                                                    setDraft(updated)
                                                }}
                                            />
                                        </div>
                                    } />

                                <div className="tracked-field">
                                    <label>Mortgage (monthly) <span className="info-icon">ⓘ<span className="info-tooltip">Derived from price, down payment %, interest rate, and amortization years</span></span></label>
                                    <span className="tracked-value">{moneyLabel(p.mortgage_monthly)}</span>
                                </div>
                                <div className="tracked-field">
                                    <label>Monthly total <span className="info-icon">ⓘ<span className="info-tooltip">Mortgage + property tax (monthly) + HOA / strata fee</span></span></label>
                                    <span className="tracked-value">{moneyLabel(p.monthly_total)}</span>
                                </div>
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Rental</h4>
                            <div className="tracked-fields">
                                <Field label="Has rental suite" viewVal={boolLabel(p.has_rental_suite)}
                                    editEl={<BoolSelect label="Has rental suite" value={draft?.has_rental_suite ?? null} onChange={v => setDraftField('has_rental_suite', v)} />} />
                                {(editMode || p.has_rental_suite !== false) && (
                                    <Field label="Rental income (monthly)" viewVal={moneyLabel(p.rental_income)}
                                        editEl={<NumInput label="Rental income (monthly)" value={draft?.rental_income ?? null} onChange={v => setDraftField('rental_income', v)} />} />
                                )}
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Nearby Schools <span className="school-source-note">(Fraser Institute rating /10)</span></h4>
                            <div className="tracked-fields">
                                <div className="tracked-field">
                                    <label>Elementary</label>
                                    {editMode ? (
                                        <div className="school-edit-row">
                                            <input className="edit-input" value={draft?.school_elementary ?? ''} onChange={e => setDraftField('school_elementary', e.target.value || null)} placeholder="School name" />
                                            <input className="edit-input edit-rating" type="number" min={0} max={10} step={0.1} value={draft?.school_elementary_rating ?? ''} onChange={e => setDraftField('school_elementary_rating', e.target.value ? Number(e.target.value) : null)} placeholder="Rating" />
                                        </div>
                                    ) : (
                                        <span className="tracked-value school-entry">
                                            {p.school_elementary ?? '—'}
                                            {p.school_elementary_rating != null && <span className="school-rating">{p.school_elementary_rating.toFixed(1)}</span>}
                                        </span>
                                    )}
                                </div>
                                <div className="tracked-field">
                                    <label>Middle</label>
                                    {editMode ? (
                                        <div className="school-edit-row">
                                            <input className="edit-input" value={draft?.school_middle ?? ''} onChange={e => setDraftField('school_middle', e.target.value || null)} placeholder="School name" />
                                            <input className="edit-input edit-rating" type="number" min={0} max={10} step={0.1} value={draft?.school_middle_rating ?? ''} onChange={e => setDraftField('school_middle_rating', e.target.value ? Number(e.target.value) : null)} placeholder="Rating" />
                                        </div>
                                    ) : (
                                        <span className="tracked-value school-entry">
                                            {p.school_middle ?? '—'}
                                            {p.school_middle_rating != null && <span className="school-rating">{p.school_middle_rating.toFixed(1)}</span>}
                                        </span>
                                    )}
                                </div>
                                <div className="tracked-field">
                                    <label>Secondary</label>
                                    {editMode ? (
                                        <div className="school-edit-row">
                                            <input className="edit-input" value={draft?.school_secondary ?? ''} onChange={e => setDraftField('school_secondary', e.target.value || null)} placeholder="School name" />
                                            <input className="edit-input edit-rating" type="number" min={0} max={10} step={0.1} value={draft?.school_secondary_rating ?? ''} onChange={e => setDraftField('school_secondary_rating', e.target.value ? Number(e.target.value) : null)} placeholder="Rating" />
                                        </div>
                                    ) : (
                                        <span className="tracked-value school-entry">
                                            {p.school_secondary ?? '—'}
                                            {p.school_secondary_rating != null && <span className="school-rating">{p.school_secondary_rating.toFixed(1)}</span>}
                                        </span>
                                    )}
                                </div>
                            </div>
                        </div>
                    </div>

                    <div className="detail-metadata">
                        <div className="meta-item">
                            <strong>Redfin:</strong>
                            {editMode ? (
                                <input
                                    className="edit-input"
                                    type="url"
                                    value={draft?.redfin_url ?? ''}
                                    onChange={e => setDraftField('redfin_url', e.target.value || null)}
                                    placeholder="https://www.redfin.ca/…"
                                />
                            ) : property.redfin_url ? (
                                <a href={property.redfin_url} target="_blank" rel="noreferrer">{property.redfin_url}</a>
                            ) : <span className="tracked-value">—</span>}
                        </div>
                        <div className="meta-item">
                            <strong>Realtor.ca:</strong>
                            {editMode ? (
                                <input
                                    className="edit-input"
                                    type="url"
                                    value={draft?.realtor_url ?? ''}
                                    onChange={e => setDraftField('realtor_url', e.target.value || null)}
                                    placeholder="https://www.realtor.ca/real-estate/…"
                                />
                            ) : property.realtor_url ? (
                                <a href={property.realtor_url} target="_blank" rel="noreferrer">{property.realtor_url}</a>
                            ) : <span className="tracked-value">—</span>}
                        </div>
                        <div className="meta-item">
                            <strong>rew.ca:</strong>
                            {editMode ? (
                                <input
                                    className="edit-input"
                                    type="url"
                                    value={draft?.rew_url ?? ''}
                                    onChange={e => setDraftField('rew_url', e.target.value || null)}
                                    placeholder="https://www.rew.ca/properties/…"
                                />
                            ) : property.rew_url ? (
                                <a href={property.rew_url} target="_blank" rel="noreferrer">{property.rew_url}</a>
                            ) : <span className="tracked-value">—</span>}
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
                            {STATUS_OPTIONS.map(s => (
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
                        onChange={e => setNotes(e.target.value)}
                        onBlur={handleNotesSave}
                        placeholder="Add personal notes about this property…"
                        disabled={notesSaving}
                    />
                    {notesSaving && <div className="notes-saving">Saving…</div>}

                    {history.length > 0 && (
                        <div className="history-panel">
                            <h3 className="notes-heading">Change History</h3>
                            <ul className="history-list">
                                {history.map(entry => (
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
