import { useEffect, useState } from 'react'
import { useNavigate } from 'react-router-dom'
import './App.css'
import type { SavedSearch } from './types'

export function ManageSearches() {
    const navigate = useNavigate()

    const [searches, setSearches] = useState<SavedSearch[]>([])
    const [editingId, setEditingId] = useState<number | null>(null)
    const [editDraft, setEditDraft] = useState<{ title: string; desc: string }>({ title: '', desc: '' })
    const [savingId, setSavingId] = useState<number | null>(null)
    const [confirmDeleteId, setConfirmDeleteId] = useState<number | null>(null)
    const [deletingId, setDeletingId] = useState<number | null>(null)

    async function fetchSearches() {
        try {
            const resp = await fetch('/api/searches')
            if (resp.ok) {
                const data: SavedSearch[] = await resp.json()
                setSearches(data)
            }
        } catch { /* non-fatal */ }
    }

    useEffect(() => { fetchSearches() }, [])

    function startEdit(s: SavedSearch) {
        setEditingId(s.id)
        setEditDraft({ title: s.title, desc: s.description ?? '' })
        setConfirmDeleteId(null)
    }

    function cancelEdit() {
        setEditingId(null)
    }

    async function handleSaveSearch(id: number) {
        setSavingId(id)
        try {
            const resp = await fetch(`/api/searches/${id}`, {
                method: 'PATCH',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ title: editDraft.title.trim(), description: editDraft.desc.trim() }),
            })
            if (resp.ok) {
                await fetchSearches()
                setEditingId(null)
            }
        } catch { /* non-fatal */ } finally {
            setSavingId(null)
        }
    }

    async function handleDeleteSearch(id: number) {
        setDeletingId(id)
        try {
            await fetch(`/api/searches/${id}`, { method: 'DELETE' })
            await fetchSearches()
            setConfirmDeleteId(null)
            if (editingId === id) setEditingId(null)
        } catch { /* non-fatal */ } finally {
            setDeletingId(null)
        }
    }

    return (
        <div className="manage-page">
            <div className="detail-nav">
                <button className="back-btn" onClick={() => navigate(-1)}>
                    <svg width="7" height="12" viewBox="0 0 7 12" fill="none" aria-hidden="true"><path d="M6 1L1 6l5 5" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" strokeLinejoin="round" /></svg>
                    Back
                </button>
                <span className="detail-nav-title">Manage Scenarios</span>
            </div>
            <div className="manage-page-content">
                {searches.map(s => {
                    const isEditing = editingId === s.id
                    const isDirty = isEditing && (editDraft.title !== s.title || editDraft.desc !== (s.description ?? ''))
                    return (
                        <div key={s.id} className={`manage-search-card${isEditing ? ' editing' : ''}`}>
                            {/* Delete button — upper right, edit mode only (hidden when confirm is showing) */}
                            {isEditing && confirmDeleteId !== s.id && (
                                <div className="manage-search-delete-corner">
                                    <button
                                        className="delete-btn"
                                        title={searches.length <= 1 ? 'Cannot delete the only scenario' : `Delete "${s.title}"`}
                                        disabled={searches.length <= 1}
                                        onClick={() => setConfirmDeleteId(s.id)}
                                    >
                                        Delete
                                    </button>
                                </div>
                            )}

                            {isEditing ? (
                                <div className="manage-search-card-fields">
                                    <input
                                        className="manage-search-edit-title"
                                        value={editDraft.title}
                                        onChange={e => setEditDraft(d => ({ ...d, title: e.target.value }))}
                                        placeholder="Scenario title"
                                        autoFocus
                                    />
                                    <textarea
                                        className="manage-search-edit-desc"
                                        value={editDraft.desc}
                                        onChange={e => setEditDraft(d => ({ ...d, desc: e.target.value }))}
                                        placeholder="Description (optional)"
                                        rows={3}
                                    />
                                </div>
                            ) : (
                                <div className="manage-search-card-info">
                                    <div className="manage-search-title">{s.title}</div>
                                    {s.description && <div className="manage-search-desc-text">{s.description}</div>}
                                </div>
                            )}
                            {confirmDeleteId === s.id ? (
                                <div className="manage-search-delete-banner">
                                    <span>Delete this scenario? Its listings will be unassigned.</span>
                                    <div className="manage-search-delete-banner-actions">
                                        <button className="cancel-btn" onClick={() => setConfirmDeleteId(null)}>Cancel</button>
                                        <button
                                            className="confirm-delete-btn"
                                            disabled={deletingId === s.id}
                                            onClick={() => handleDeleteSearch(s.id)}
                                        >
                                            {deletingId === s.id ? 'Deleting…' : 'Delete'}
                                        </button>
                                    </div>
                                </div>
                            ) : (
                                <div className="manage-search-card-footer">
                                    <span className="manage-search-count">
                                        {s.listing_count} {s.listing_count === 1 ? 'listing' : 'listings'}
                                    </span>
                                    <div className="manage-search-card-actions">
                                        {isEditing ? (
                                            <>
                                                <button
                                                    className="save-btn"
                                                    disabled={savingId === s.id || !isDirty || !editDraft.title.trim()}
                                                    onClick={() => handleSaveSearch(s.id)}
                                                >
                                                    {savingId === s.id ? 'Saving…' : 'Save'}
                                                </button>
                                                <button className="cancel-btn" onClick={cancelEdit}>Cancel</button>
                                            </>
                                        ) : (
                                            <button className="edit-btn" onClick={() => startEdit(s)}>Edit</button>
                                        )}
                                    </div>
                                </div>
                            )}
                        </div>
                    )
                })}
            </div>
        </div>
    )
}
