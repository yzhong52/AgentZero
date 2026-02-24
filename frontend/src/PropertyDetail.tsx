import { useEffect, useRef, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import { marked } from 'marked'
import { emojify, get as getEmoji, search as searchEmoji } from 'node-emoji'
import type { Property } from './types'
import { STATUS_OPTIONS, STATUS_COLORS } from './constants'

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

function calcMonthlyTotal(mortgageMonthly: number | null, propertyTaxAnnual: number | null, hoaMonthly: number | null): number | null {
    if (mortgageMonthly == null && propertyTaxAnnual == null && hoaMonthly == null) return null
    const taxMonthly = propertyTaxAnnual != null ? Math.floor(propertyTaxAnnual / 12) : 0
    return (mortgageMonthly ?? 0) + taxMonthly + (hoaMonthly ?? 0)
}

function calcInitialMonthlyInterest(price: number | null, downPct: number, annualRate: number): number | null {
    if (!price) return null
    const loan = price * (1 - downPct)
    if (loan <= 0) return 0
    return Math.round((loan * annualRate) / 12)
}

function moneyPart(v: number | null): string {
    return `$${(v ?? 0).toLocaleString()}`
}

type EmojiSuggestion = {
    name: string
    emoji: string
}

const EMOJI_ALIAS_TO_CANONICAL: Record<string, string> = {
    warn: 'warning',
    ok: 'white_check_mark',
    check: 'white_check_mark',
    xmark: 'x',
    nope: 'x',
}

const POPULAR_EMOJI_NAMES = [
    'warning',
    'white_check_mark',
    'x',
    'fire',
    'rocket',
    'hourglass',
    'eyes',
    'bulb',
    'star',
    'moneybag',
    'house',
    'key',
]

const POPULAR_EMOJI_SUGGESTIONS: EmojiSuggestion[] = POPULAR_EMOJI_NAMES
    .map(name => ({ name, emoji: getEmoji(name) }))
    .filter((item): item is EmojiSuggestion => typeof item.emoji === 'string')

function normalizeEmojiAliases(input: string): string {
    return input.replace(/:([a-z0-9_+-]+):/gi, (match, shortcode: string) => {
        const canonical = EMOJI_ALIAS_TO_CANONICAL[shortcode.toLowerCase()]
        return canonical ? `:${canonical}:` : match
    })
}

function replaceEmojiShortcodes(input: string): string {
    return emojify(normalizeEmojiAliases(input))
}

function detectEmojiQuery(input: string, caretPos: number): { start: number; end: number; query: string } | null {
    if (caretPos < 0 || caretPos > input.length) return null

    const beforeCaret = input.slice(0, caretPos)
    const start = beforeCaret.lastIndexOf(':')
    if (start < 0) return null

    if (start > 0) {
        const prevChar = input[start - 1]
        if (!/[\s([{"']/.test(prevChar)) return null
    }

    const query = input.slice(start + 1, caretPos)
    if (!/^[a-z0-9_+-]*$/i.test(query)) return null
    if (query.includes(':')) return null

    return { start, end: caretPos, query: query.toLowerCase() }
}

function buildEmojiSuggestions(query: string): EmojiSuggestion[] {
    if (!query) return POPULAR_EMOJI_SUGGESTIONS

    const canonical = EMOJI_ALIAS_TO_CANONICAL[query]
    const aliasSuggestion = canonical
        ? getEmoji(canonical)
            ? [{ name: query, emoji: getEmoji(canonical)! }]
            : []
        : []

    const results = searchEmoji(query)
        .slice(0, 20)
        .map(r => ({ name: r.name, emoji: r.emoji }))

    const deduped = new Map<string, EmojiSuggestion>()
    for (const item of [...aliasSuggestion, ...results]) {
        const key = `${item.name}:${item.emoji}`
        if (!deduped.has(key)) deduped.set(key, item)
        if (deduped.size >= 8) break
    }
    return Array.from(deduped.values())
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
    { key: 'title', label: 'Title' },
    { key: 'price', label: 'Price' },
    { key: 'street_address', label: 'Address' },
    { key: 'city', label: 'City' },
    { key: 'region', label: 'Region' },
    { key: 'postal_code', label: 'Postal code' },
    { key: 'bedrooms', label: 'Bedrooms' },
    { key: 'bathrooms', label: 'Bathrooms' },
    { key: 'sqft', label: 'Sqft' },
    { key: 'year_built', label: 'Year built' },
    { key: 'land_sqft', label: 'Land sqft' },
    { key: 'parking_garage', label: 'Garage' },
    { key: 'ac', label: 'Air conditioning' },
    { key: 'radiant_floor_heating', label: 'Radiant heating' },
    { key: 'property_tax', label: 'Property tax (annual)' },
    { key: 'hoa_monthly', label: 'HOA / Strata (monthly)' },
    { key: 'school_elementary', label: 'Elementary school' },
    { key: 'school_middle', label: 'Middle school' },
    { key: 'school_secondary', label: 'Secondary school' },
]

function buildDiff(stored: Property, fresh: Property): DiffEntry[] {
    return DIFF_FIELDS
        .filter(f => str(stored[f.key]) !== str(fresh[f.key]))
        .map(f => ({ field: f.label, old: str(stored[f.key]), fresh: str(fresh[f.key]) }))
}

// ── Build details payload (all editable fields) ───────────────────────────────

function toUserDetails(p: Property) {
    return {
        title: p.title,
        price: p.price,
        price_currency: p.price_currency,
        offer_price: p.offer_price,
        street_address: p.street_address,
        city: p.city,
        region: p.region,
        postal_code: p.postal_code,
        bedrooms: p.bedrooms,
        bathrooms: p.bathrooms,
        sqft: p.sqft,
        year_built: p.year_built,
        parking_garage: p.parking_garage,
        parking_covered: p.parking_covered,
        parking_open: p.parking_open,
        land_sqft: p.land_sqft,
        property_tax: p.property_tax,
        skytrain_station: p.skytrain_station,
        skytrain_walk_min: p.skytrain_walk_min,
        radiant_floor_heating: p.radiant_floor_heating,
        ac: p.ac,
        down_payment_pct: p.down_payment_pct,
        mortgage_interest_rate: p.mortgage_interest_rate,
        amortization_years: p.amortization_years,
        mortgage_monthly: p.mortgage_monthly,
        hoa_monthly: p.hoa_monthly,
        monthly_total: p.monthly_total,
        monthly_cost: p.monthly_cost,
        has_rental_suite: p.has_rental_suite,
        rental_income: p.rental_income,
        status: p.status,
        school_elementary: p.school_elementary,
        school_elementary_rating: p.school_elementary_rating,
        school_middle: p.school_middle,
        school_middle_rating: p.school_middle_rating,
        school_secondary: p.school_secondary,
        school_secondary_rating: p.school_secondary_rating,
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
    const [notesEditing, setNotesEditing] = useState(false)
    const notesInputRef = useRef<HTMLTextAreaElement>(null)
    const [emojiSuggestions, setEmojiSuggestions] = useState<EmojiSuggestion[]>([])
    const [emojiSuggestActiveIdx, setEmojiSuggestActiveIdx] = useState(0)
    const [emojiSuggestRange, setEmojiSuggestRange] = useState<{ start: number; end: number } | null>(null)

    // Title (inline header)
    const [titleDraft, setTitleDraft] = useState<string>('')

    // Edit mode
    const [editMode, setEditMode] = useState(false)
    const [draft, setDraft] = useState<Property | null>(null)
    const [saving, setSaving] = useState(false)
    const [financeEditMode, setFinanceEditMode] = useState(false)
    const [financeSaving, setFinanceSaving] = useState(false)
    const [financeDraft, setFinanceDraft] = useState<Property | null>(null)

    // Refresh preview
    const [previewing, setPreviewing] = useState(false)
    const [applying, setApplying] = useState(false)
    const [diffModal, setDiffModal] = useState<DiffEntry[] | null>(null)

    // Delete
    const [deleting, setDeleting] = useState(false)

    // URL draft (right panel — always editable, independent of main edit mode)
    const [urlDraft, setUrlDraft] = useState<{
        redfin_url: string | null
        realtor_url: string | null
        rew_url: string | null
        zillow_url: string | null
    }>({ redfin_url: null, realtor_url: null, rew_url: null, zillow_url: null })
    const [editingUrlKey, setEditingUrlKey] = useState<'redfin_url' | 'realtor_url' | 'rew_url' | 'zillow_url' | null>(null)
    const [urlsSaving, setUrlsSaving] = useState(false)
    const [urlsExpanded, setUrlsExpanded] = useState(false)

    // History expand
    const [historyExpanded, setHistoryExpanded] = useState(false)

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
            setTitleDraft(p.title ?? '')
            setUrlDraft({ redfin_url: p.redfin_url, realtor_url: p.realtor_url, rew_url: p.rew_url, zillow_url: p.zillow_url })

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
        setFinanceEditMode(false)
        setFinanceDraft(null)
    }

    function setDraftField<K extends keyof Property>(key: K, val: Property[K]) {
        setDraft(d => d ? { ...d, [key]: val } : d)
    }

    /** Recalculate mortgage_monthly in draft whenever offer price / price or params change. */
    function recalcMortgage(d: Property): Property {
        const initialInterest = calcInitialMonthlyInterest(
            d.offer_price ?? d.price,
            d.down_payment_pct ?? 0.20,
            d.mortgage_interest_rate ?? 0.05,
        )
        const monthly = calcMortgage(
            d.offer_price ?? d.price,
            d.down_payment_pct ?? 0.20,
            d.mortgage_interest_rate ?? 0.05,
            d.amortization_years ?? 25,
        )
        return {
            ...d,
            mortgage_monthly: monthly,
            monthly_total: calcMonthlyTotal(monthly, d.property_tax, d.hoa_monthly),
            monthly_cost: calcMonthlyTotal(initialInterest, d.property_tax, d.hoa_monthly),
        }
    }

    function enterFinanceEdit() {
        if (!property) return
        setFinanceDraft(recalcMortgage({ ...property }))
        setFinanceEditMode(true)
    }

    function cancelFinanceEdit() {
        setFinanceEditMode(false)
        setFinanceDraft(null)
    }

    async function saveFinanceEdits() {
        if (!financeDraft || !property) return
        setFinanceSaving(true)
        setError(null)
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(toUserDetails(financeDraft)),
            })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty({ ...updated, images: property.images })
            if (editMode) {
                setDraft(updated)
            }
            setFinanceEditMode(false)
            setFinanceDraft(null)
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setFinanceSaving(false)
        }
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

    // ── URL save (right panel) ────────────────────────────────────────────────

    async function saveUrls(): Promise<boolean> {
        if (!property) return false
        setUrlsSaving(true)
        setError(null)
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify(urlDraft),
            })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty({ ...updated, images: property.images })
            setUrlDraft({ redfin_url: updated.redfin_url, realtor_url: updated.realtor_url, rew_url: updated.rew_url, zillow_url: updated.zillow_url })
            setEditingUrlKey(null)
            return true
        } catch (err: any) {
            setError(err?.message || String(err))
            return false
        } finally {
            setUrlsSaving(false)
        }
    }

    // ── Refresh with preview diff ─────────────────────────────────────────────

    async function handleRefreshPreview() {
        if (!property) return
        setError(null)
        setRefreshMsg(null)
        // Save any URL edits before refreshing
        const hasChanges = urlDraft.redfin_url !== property.redfin_url ||
            urlDraft.realtor_url !== property.realtor_url ||
            urlDraft.rew_url !== property.rew_url ||
            urlDraft.zillow_url !== property.zillow_url
        if (hasChanges) {
            const ok = await saveUrls()
            if (!ok) return
        }
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
            const resp = await fetch(`/api/listings/${property.id}/refresh`, { method: 'PUT' })
            if (!resp.ok) throw new Error(await resp.text())
            const updated: Property = await resp.json()
            setProperty(updated)
            setTitleDraft(updated.title ?? '')
            setUrlDraft({ redfin_url: updated.redfin_url, realtor_url: updated.realtor_url, rew_url: updated.rew_url, zillow_url: updated.zillow_url })
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
        const normalizedNotes = replaceEmojiShortcodes(notes)
        setNotes(normalizedNotes)
        setEmojiSuggestions([])
        setEmojiSuggestRange(null)
        try {
            const resp = await fetch(`/api/listings/${property.id}/notes`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ notes: normalizedNotes || null }),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        } finally {
            setNotesSaving(false)
        }
    }

    function refreshEmojiSuggestions(inputValue: string, caretPos: number) {
        const trigger = detectEmojiQuery(inputValue, caretPos)
        if (!trigger) {
            setEmojiSuggestions([])
            setEmojiSuggestRange(null)
            setEmojiSuggestActiveIdx(0)
            return
        }

        const suggestions = buildEmojiSuggestions(trigger.query)
        setEmojiSuggestions(suggestions)
        setEmojiSuggestRange({ start: trigger.start, end: trigger.end })
        setEmojiSuggestActiveIdx(0)
    }

    function insertEmojiSuggestion(suggestion: EmojiSuggestion) {
        if (!emojiSuggestRange) return
        const next = `${notes.slice(0, emojiSuggestRange.start)}${suggestion.emoji}${notes.slice(emojiSuggestRange.end)}`
        const nextCaret = emojiSuggestRange.start + suggestion.emoji.length
        setNotes(next)
        setEmojiSuggestions([])
        setEmojiSuggestRange(null)
        requestAnimationFrame(() => {
            const textarea = notesInputRef.current
            if (!textarea) return
            textarea.focus()
            textarea.setSelectionRange(nextCaret, nextCaret)
        })
    }

    // ── Title (inline) ─────────────────────────────────────────────────────────

    async function handleTitleSave() {
        if (!property || !titleDraft.trim()) return
        const newTitle = titleDraft.trim()
        setProperty({ ...property, title: newTitle })
        try {
            const resp = await fetch(`/api/listings/${property.id}/details`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ title: newTitle }),
            })
            if (!resp.ok) throw new Error(await resp.text())
        } catch (err: any) {
            setError(err?.message || String(err))
        }
    }

    // ── Status ────────────────────────────────────────────────────────────────

    async function handleStatusChange(newStatus: string) {
        if (!property) return
        const updated = { ...property, status: newStatus }
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
            const resp = await fetch(`/api/listings/${property.id}/delete`, { method: 'DELETE' })
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
    const finance = financeEditMode && financeDraft ? financeDraft : property
    const financeBasePrice = finance.offer_price ?? finance.price
    const initialMonthlyInterest = calcInitialMonthlyInterest(
        financeBasePrice,
        finance.down_payment_pct ?? 0.20,
        finance.mortgage_interest_rate ?? 0.05,
    )
    const monthlyTotalDerived = calcMonthlyTotal(finance.mortgage_monthly, finance.property_tax, finance.hoa_monthly)
    const monthlyCost = calcMonthlyTotal(initialMonthlyInterest, finance.property_tax, finance.hoa_monthly)
    const taxMonthly = finance.property_tax != null ? Math.floor(finance.property_tax / 12) : 0
    const hoaMonthly = finance.hoa_monthly ?? 0
    const effectiveOfferPrice = finance.offer_price ?? finance.price
    const hasCustomOfferPrice = finance.offer_price != null && finance.price != null && finance.offer_price !== finance.price
    const hasUrlChanges = urlDraft.redfin_url !== property.redfin_url ||
        urlDraft.realtor_url !== property.realtor_url ||
        urlDraft.rew_url !== property.rew_url ||
        urlDraft.zillow_url !== property.zillow_url
    const externalUrlRows: Array<{ key: 'redfin_url' | 'realtor_url' | 'rew_url' | 'zillow_url'; label: string; placeholder: string }> = [
        { key: 'redfin_url', label: 'Redfin', placeholder: 'https://www.redfin.ca/…' },
        { key: 'realtor_url', label: 'Realtor.ca', placeholder: 'https://www.realtor.ca/…' },
        { key: 'rew_url', label: 'rew.ca', placeholder: 'https://www.rew.ca/properties/…' },
        { key: 'zillow_url', label: 'Zillow', placeholder: 'https://www.zillow.com/homedetails/…' },
    ]
    const monthlyTotalBreakdown = `${moneyPart(finance.mortgage_monthly)} mortgage + ${moneyPart(taxMonthly)} tax + ${moneyPart(hoaMonthly)} HOA`
    const monthlyCostBreakdown = `${moneyPart(initialMonthlyInterest)} initial interest + ${moneyPart(taxMonthly)} tax + ${moneyPart(hoaMonthly)} HOA`

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
                            <span className="lightbox-title">{property.title}</span>
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
                            className="title-input"
                            value={titleDraft}
                            onChange={e => setTitleDraft(e.target.value)}
                            onBlur={handleTitleSave}
                            placeholder="(no title)"
                            aria-label="Property title"
                        />
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
                                    <Field label="Square Feet" viewVal={p.sqft != null ? p.sqft.toLocaleString() : '—'}
                                        editEl={<NumInput label="Square Feet" value={draft?.sqft ?? null} onChange={v => setDraftField('sqft', v)} />} />
                                )}
                                {(p.year_built != null || editMode) && (
                                    <Field label="Year Built" viewVal={numLabel(p.year_built)}
                                        editEl={<NumInput label="Year Built" value={draft?.year_built ?? null} onChange={v => setDraftField('year_built', v)} />} />
                                )}
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Parking</h4>
                            <div className="tracked-fields">
                                <Field label="Garage (Indoor)" viewVal={numLabel(p.parking_garage)}
                                    editEl={<NumInput label="Garage (Indoor)" value={draft?.parking_garage ?? null} onChange={v => setDraftField('parking_garage', v)} />} />
                                <Field label="Covered Outdoor" viewVal={numLabel(p.parking_covered)}
                                    editEl={<NumInput label="Covered Outdoor" value={draft?.parking_covered ?? null} onChange={v => setDraftField('parking_covered', v)} />} />
                                <Field label="Open Outdoor" viewVal={numLabel(p.parking_open)}
                                    editEl={<NumInput label="Open Outdoor" value={draft?.parking_open ?? null} onChange={v => setDraftField('parking_open', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Land</h4>
                            <div className="tracked-fields">
                                <Field label="Land Size (Sqft)" viewVal={numLabel(p.land_sqft, ' sqft')}
                                    editEl={<NumInput label="Land Size (Sqft)" value={draft?.land_sqft ?? null} onChange={v => setDraftField('land_sqft', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Transit</h4>
                            <div className="tracked-fields">
                                <Field label="Closest Skytrain Station" viewVal={p.skytrain_station ?? '—'}
                                    editEl={<TextInput label="Closest Skytrain Station" value={draft?.skytrain_station ?? null} onChange={v => setDraftField('skytrain_station', v)} />} />
                                <Field label="Walk Time (Min)" viewVal={numLabel(p.skytrain_walk_min, ' min')}
                                    editEl={<NumInput label="Walk Time (Min)" value={draft?.skytrain_walk_min ?? null} onChange={v => setDraftField('skytrain_walk_min', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Features</h4>
                            <div className="tracked-fields">
                                <Field label="Radiant Floor Heating" viewVal={boolLabel(p.radiant_floor_heating)}
                                    editEl={<BoolSelect label="Radiant Floor Heating" value={draft?.radiant_floor_heating ?? null} onChange={v => setDraftField('radiant_floor_heating', v)} />} />
                                <Field label="Air Conditioning" viewVal={boolLabel(p.ac)}
                                    editEl={<BoolSelect label="Air Conditioning" value={draft?.ac ?? null} onChange={v => setDraftField('ac', v)} />} />
                            </div>
                        </div>

                        <div className="tracked-group">
                            <h4>Rental</h4>
                            <div className="tracked-fields">
                                <Field label="Has Rental Suite" viewVal={boolLabel(p.has_rental_suite)}
                                    editEl={<BoolSelect label="Has Rental Suite" value={draft?.has_rental_suite ?? null} onChange={v => setDraftField('has_rental_suite', v)} />} />
                                {(editMode || p.has_rental_suite !== false) && (
                                    <Field label="Rental Income (Monthly)" viewVal={moneyLabel(p.rental_income)}
                                        editEl={<NumInput label="Rental Income (Monthly)" value={draft?.rental_income ?? null} onChange={v => setDraftField('rental_income', v)} />} />
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

                    <div className="location-card">
                        <h3>Location</h3>
                        <div className="tracked-fields location-fields">
                            <Field label="Street Address" viewVal={p.street_address ?? '—'}
                                editEl={<TextInput label="Street Address" value={draft?.street_address ?? null} onChange={v => setDraftField('street_address', v)} />} />
                            <Field label="City" viewVal={p.city ?? '—'}
                                editEl={<TextInput label="City" value={draft?.city ?? null} onChange={v => setDraftField('city', v)} />} />
                            <Field label="Region / Province" viewVal={p.region ?? '—'}
                                editEl={<TextInput label="Region / Province" value={draft?.region ?? null} onChange={v => setDraftField('region', v)} />} />
                            <Field label="Postal Code" viewVal={p.postal_code ?? '—'}
                                editEl={<TextInput label="Postal Code" value={draft?.postal_code ?? null} onChange={v => setDraftField('postal_code', v)} />} />
                        </div>

                        {property.lat != null && property.lon != null && (
                            <div className="map-preview">
                                <iframe
                                    title="Property Location"
                                    src={`https://maps.google.com/maps?q=${property.lat},${property.lon}&z=15&output=embed`}
                                    loading="lazy"
                                    referrerPolicy="no-referrer-when-downgrade"
                                />
                            </div>
                        )}
                    </div>

                    <div className="offer-finance-card">
                        <div className="offer-finance-header">
                            <h3>Offer &amp; Finance</h3>
                            {financeEditMode && (
                                <div className="offer-finance-actions">
                                    <button className="cancel-btn" onClick={cancelFinanceEdit} disabled={financeSaving}>Cancel</button>
                                    <button className="save-btn" onClick={saveFinanceEdits} disabled={financeSaving}>
                                        {financeSaving ? 'Saving…' : 'Save'}
                                    </button>
                                </div>
                            )}
                        </div>

                        <div className="offer-finance-row offer-finance-row-1">
                            <div className="tracked-field">
                                <label>Target Offer Price <span className="info-icon">ⓘ<span className="info-tooltip">Used as the base for mortgage calculations. Leave blank to use the listing price.</span></span></label>
                                {financeEditMode ? (
                                    <div className="target-offer-edit-row">
                                        <input
                                            className="edit-input target-offer-input"
                                            type="number"
                                            value={financeDraft?.offer_price ?? ''}
                                            onChange={e => {
                                                const updated = recalcMortgage({ ...financeDraft!, offer_price: e.target.value ? Number(e.target.value) : null })
                                                setFinanceDraft(updated)
                                            }}
                                            placeholder="Defaults to listing price"
                                        />
                                        {hasCustomOfferPrice && (
                                            <span className="offer-price-original">
                                                {formatPrice(finance.price, finance.price_currency)}
                                            </span>
                                        )}
                                    </div>
                                ) : (
                                    <button className="offer-price-btn" onClick={enterFinanceEdit}>
                                        <span className="offer-price-value">
                                            {formatPrice(effectiveOfferPrice, finance.price_currency) ?? '—'}
                                        </span>
                                        {hasCustomOfferPrice && (
                                            <span className="offer-price-original">
                                                {formatPrice(finance.price, finance.price_currency)}
                                            </span>
                                        )}
                                    </button>
                                )}
                            </div>
                        </div>

                        <div className="offer-finance-row offer-finance-row-3">
                            <div className="tracked-field">
                                <label>Down Payment %</label>
                                {financeEditMode ? (
                                    <input
                                        className="edit-input"
                                        type="number"
                                        min={0} max={100} step={1}
                                        value={financeDraft?.down_payment_pct != null ? (financeDraft.down_payment_pct * 100).toFixed(0) : ''}
                                        onChange={e => {
                                            const pct = e.target.value ? Number(e.target.value) / 100 : null
                                            const updated = recalcMortgage({ ...financeDraft!, down_payment_pct: pct })
                                            setFinanceDraft(updated)
                                        }}
                                    />
                                ) : (
                                    <span className="tracked-value">{finance.down_payment_pct != null ? `${(finance.down_payment_pct * 100).toFixed(0)}%` : '—'}</span>
                                )}
                            </div>

                            <div className="tracked-field">
                                <label>Mortgage Rate %</label>
                                {financeEditMode ? (
                                    <input
                                        className="edit-input"
                                        type="number"
                                        min={0} max={30} step={0.01}
                                        value={financeDraft?.mortgage_interest_rate != null ? (financeDraft.mortgage_interest_rate * 100).toFixed(2) : ''}
                                        onChange={e => {
                                            const rate = e.target.value ? Number(e.target.value) / 100 : null
                                            const updated = recalcMortgage({ ...financeDraft!, mortgage_interest_rate: rate })
                                            setFinanceDraft(updated)
                                        }}
                                    />
                                ) : (
                                    <span className="tracked-value">{finance.mortgage_interest_rate != null ? `${(finance.mortgage_interest_rate * 100).toFixed(2)}%` : '—'}</span>
                                )}
                            </div>

                            <div className="tracked-field">
                                <label>Amortization (years)</label>
                                {financeEditMode ? (
                                    <input
                                        className="edit-input"
                                        type="number"
                                        min={1} max={40} step={1}
                                        value={financeDraft?.amortization_years ?? ''}
                                        onChange={e => {
                                            const yrs = e.target.value ? Number(e.target.value) : null
                                            const updated = recalcMortgage({ ...financeDraft!, amortization_years: yrs })
                                            setFinanceDraft(updated)
                                        }}
                                    />
                                ) : (
                                    <span className="tracked-value">{numLabel(finance.amortization_years, ' yr')}</span>
                                )}
                            </div>
                        </div>

                        <div className="offer-finance-row offer-finance-row-3">
                            <div className="tracked-field">
                                <label>Property Tax (Annual)</label>
                                {financeEditMode ? (
                                    <input
                                        className="edit-input"
                                        type="number"
                                        value={financeDraft?.property_tax ?? ''}
                                        onChange={e => {
                                            const updated = recalcMortgage({ ...financeDraft!, property_tax: e.target.value ? Number(e.target.value) : null })
                                            setFinanceDraft(updated)
                                        }}
                                    />
                                ) : (
                                    <span className="tracked-value">{moneyLabel(finance.property_tax)}</span>
                                )}
                            </div>

                            <div className="tracked-field">
                                <label>HOA / Strata (monthly)</label>
                                {financeEditMode ? (
                                    <input
                                        className="edit-input"
                                        type="number"
                                        value={financeDraft?.hoa_monthly ?? ''}
                                        onChange={e => {
                                            const updated = recalcMortgage({ ...financeDraft!, hoa_monthly: e.target.value ? Number(e.target.value) : null })
                                            setFinanceDraft(updated)
                                        }}
                                    />
                                ) : (
                                    <span className="tracked-value">{moneyLabel(finance.hoa_monthly)}</span>
                                )}
                            </div>

                            <div className="tracked-field offer-finance-spacer" aria-hidden="true" />
                        </div>

                        <div className="offer-finance-row offer-finance-row-3">
                            <div className="tracked-field">
                                <label>Mortgage (Monthly) <span className="info-icon">ⓘ<span className="info-tooltip">Derived from offer price (or listing price), down payment %, interest rate, and amortization years</span></span></label>
                                <span className="tracked-value">{moneyLabel(finance.mortgage_monthly)}</span>
                            </div>
                            <div className="tracked-field">
                                <label>Monthly Total <span className="info-icon">ⓘ<span className="info-tooltip">{monthlyTotalBreakdown}</span></span></label>
                                <span className="tracked-value">{moneyLabel(monthlyTotalDerived)}</span>
                            </div>
                            <div className="tracked-field">
                                <label>Monthly Cost <span className="info-icon">ⓘ<span className="info-tooltip">{monthlyCostBreakdown}</span></span></label>
                                <span className="tracked-value">{moneyLabel(monthlyCost)}</span>
                            </div>
                        </div>
                    </div>
                </div>

                <div className="notes-panel">
                    <div className="status-picker right-panel-section">
                        <h3 className="notes-heading">Status</h3>
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

                    <div className="right-panel-section">
                        <h3 className="notes-heading">My Notes</h3>
                        {notesEditing ? (
                            <div className="notes-edit-wrap">
                                <textarea
                                    ref={notesInputRef}
                                    className="notes-textarea"
                                    value={notes}
                                    onChange={e => {
                                        const next = replaceEmojiShortcodes(e.target.value)
                                        setNotes(next)
                                        refreshEmojiSuggestions(next, e.target.selectionStart ?? next.length)
                                    }}
                                    onClick={e => refreshEmojiSuggestions(notes, (e.target as HTMLTextAreaElement).selectionStart ?? notes.length)}
                                    onKeyUp={e => {
                                        if (e.key === 'ArrowDown' || e.key === 'ArrowUp' || e.key === 'Enter' || e.key === 'Tab' || e.key === 'Escape') {
                                            return
                                        }
                                        refreshEmojiSuggestions(notes, (e.target as HTMLTextAreaElement).selectionStart ?? notes.length)
                                    }}
                                    onKeyDown={e => {
                                        if (emojiSuggestions.length === 0) return
                                        if (e.key === 'ArrowDown') {
                                            e.preventDefault()
                                            setEmojiSuggestActiveIdx(i => (i + 1) % emojiSuggestions.length)
                                            return
                                        }
                                        if (e.key === 'ArrowUp') {
                                            e.preventDefault()
                                            setEmojiSuggestActiveIdx(i => (i - 1 + emojiSuggestions.length) % emojiSuggestions.length)
                                            return
                                        }
                                        if (e.key === 'Enter' || e.key === 'Tab') {
                                            e.preventDefault()
                                            const picked = emojiSuggestions[emojiSuggestActiveIdx] ?? emojiSuggestions[0]
                                            if (picked) insertEmojiSuggestion(picked)
                                            return
                                        }
                                        if (e.key === 'Escape') {
                                            e.preventDefault()
                                            setEmojiSuggestions([])
                                            setEmojiSuggestRange(null)
                                        }
                                    }}
                                    onBlur={() => {
                                        setEmojiSuggestions([])
                                        setEmojiSuggestRange(null)
                                        setNotesEditing(false)
                                        handleNotesSave()
                                    }}
                                    placeholder="Add personal notes… (supports markdown, emoji shortcodes like :warning:)"
                                    disabled={notesSaving}
                                    autoFocus
                                />
                                {emojiSuggestions.length > 0 && (
                                    <div className="emoji-suggest" role="listbox" aria-label="Emoji suggestions">
                                        {emojiSuggestions.map((item, idx) => (
                                            <button
                                                key={`${item.name}-${item.emoji}`}
                                                type="button"
                                                className={`emoji-suggest-item${idx === emojiSuggestActiveIdx ? ' active' : ''}`}
                                                onMouseDown={e => {
                                                    e.preventDefault()
                                                    insertEmojiSuggestion(item)
                                                }}
                                                title={`:${item.name}:`}
                                            >
                                                <span className="emoji-suggest-glyph">{item.emoji}</span>
                                                <span className="emoji-suggest-name">:{item.name}:</span>
                                            </button>
                                        ))}
                                    </div>
                                )}
                            </div>
                        ) : (
                            <div
                                className={`notes-display${notes ? '' : ' notes-display-empty'}`}
                                onClick={() => setNotesEditing(true)}
                                title="Click to edit"
                            >
                                {notes
                                    ? <div dangerouslySetInnerHTML={{ __html: marked(notes) as string }} />
                                    : <span>Add personal notes…</span>
                                }
                            </div>
                        )}
                        {notesSaving && <div className="notes-saving">Saving…</div>}
                    </div>

                    <div className="source-urls-panel right-panel-section">
                        <div className="source-urls-header">
                            <h3 className="notes-heading">External URLs</h3>
                            <button
                                className="refresh-btn"
                                onClick={handleRefreshPreview}
                                disabled={previewing || editMode || urlsSaving}
                                title="Preview changes from source (saves URL edits first)"
                            >
                                {previewing ? '⟳ Checking…' : urlsSaving ? 'Saving…' : '⟳ Refresh'}
                            </button>
                        </div>
                        {(() => {
                            const filledRows = externalUrlRows.filter(({ key }) => urlDraft[key])
                            const hiddenCount = externalUrlRows.length - filledRows.length
                            const visibleRows = urlsExpanded ? externalUrlRows : filledRows
                            return <>
                                {visibleRows.map(({ key, label, placeholder }) => {
                                    const currentValue = urlDraft[key]
                                    const isEditing = editingUrlKey === key
                                    return (
                                        <div className="source-url-field" key={key}>
                                            <label>{label}</label>
                                            {isEditing ? (
                                                <div className="source-url-edit-row">
                                                    <input
                                                        className="edit-input"
                                                        type="url"
                                                        value={currentValue ?? ''}
                                                        onChange={e => setUrlDraft(d => ({ ...d, [key]: e.target.value || null }))}
                                                        placeholder={placeholder}
                                                    />
                                                    <button
                                                        className="source-url-edit-btn"
                                                        onClick={() => setEditingUrlKey(null)}
                                                        title="Done editing"
                                                        type="button"
                                                    >
                                                        ✓
                                                    </button>
                                                </div>
                                            ) : (
                                                <div className="source-url-line">
                                                    {currentValue ? (
                                                        <a className="source-url-link" href={currentValue} target="_blank" rel="noreferrer">
                                                            {currentValue}
                                                        </a>
                                                    ) : (
                                                        <span className="source-url-empty">—</span>
                                                    )}
                                                    <button
                                                        className="source-url-edit-btn"
                                                        onClick={() => setEditingUrlKey(key)}
                                                        title={`Edit ${label} URL`}
                                                        type="button"
                                                    >
                                                        <svg className="source-url-edit-icon" viewBox="0 0 24 24" fill="none" aria-hidden="true">
                                                            <path
                                                                d="M4 20l4.7-1 9.3-9.3a1.4 1.4 0 0 0 0-2l-1.7-1.7a1.4 1.4 0 0 0-2 0L5 15.3 4 20z"
                                                                stroke="currentColor"
                                                                strokeWidth="1.8"
                                                                strokeLinecap="round"
                                                                strokeLinejoin="round"
                                                            />
                                                            <path
                                                                d="M13.2 6.8l4 4"
                                                                stroke="currentColor"
                                                                strokeWidth="1.8"
                                                                strokeLinecap="round"
                                                            />
                                                        </svg>
                                                    </button>
                                                </div>
                                            )}
                                        </div>
                                    )
                                })}
                                {!urlsExpanded && hiddenCount > 0 && (
                                    <button className="panel-more-btn" onClick={() => setUrlsExpanded(true)}>
                                        + {hiddenCount} more
                                    </button>
                                )}
                                {urlsExpanded && (
                                    <button className="panel-more-btn" onClick={() => { setUrlsExpanded(false); setEditingUrlKey(null) }}>
                                        Show less
                                    </button>
                                )}
                            </>
                        })()}
                        {hasUrlChanges && (
                            <button className="save-btn save-urls-btn" onClick={saveUrls} disabled={urlsSaving}>
                                {urlsSaving ? 'Saving…' : 'Save URLs'}
                            </button>
                        )}
                    </div>

                    {history.length > 0 && (
                        <div className="history-panel right-panel-section">
                            <h3 className="notes-heading">Change History</h3>
                            <ul className="history-list">
                                {(historyExpanded ? history : history.slice(0, 1)).map(entry => (
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
                            {history.length > 1 && (
                                <button className="panel-more-btn" onClick={() => setHistoryExpanded(h => !h)}>
                                    {historyExpanded ? 'Show less' : `+ ${history.length - 1} more`}
                                </button>
                            )}
                        </div>
                    )}

                    <div className="listing-timestamps right-panel-section">
                        <span>Watched since: {new Date(property.created_at).toLocaleDateString('en-CA', { month: 'short', day: 'numeric', year: 'numeric' })}</span>
                        <span>Last refreshed: {property.updated_at ? new Date(property.updated_at).toLocaleDateString('en-CA', { month: 'short', day: 'numeric', year: 'numeric' }) : '—'}</span>
                    </div>
                </div>
            </div>
        </div>
    )
}
