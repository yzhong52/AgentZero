import { useEffect, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import type { Property, ImageEntry } from './App'

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

            <div className="detail-content">
                <div className="detail-header">
                    <h1>{property.title}</h1>
                    <div className="detail-price">{formatPrice(property.price, property.price_currency)}</div>
                </div>

                {address && <div className="detail-address">{address}</div>}

                <div className="detail-specs">
                    {property.bedrooms != null && <div className="spec"><strong>{property.bedrooms}</strong> Bedrooms</div>}
                    {property.bathrooms != null && <div className="spec"><strong>{property.bathrooms}</strong> Bathrooms</div>}
                    {property.sqft != null && <div className="spec"><strong>{property.sqft.toLocaleString()}</strong> sqft</div>}
                    {property.year_built != null && <div className="spec"><strong>Built {property.year_built}</strong></div>}
                </div>

                {property.description && (
                    <div className="detail-description">
                        <h3>Description</h3>
                        <p>{property.description}</p>
                    </div>
                )}

                <div className="detail-metadata">
                    <div className="meta-item">
                        <strong>URL:</strong>
                        <a href={property.url} target="_blank" rel="noreferrer">{property.url}</a>
                    </div>
                    <div className="meta-item">
                        <strong>Latitude:</strong> {property.lat ?? 'N/A'}
                    </div>
                    <div className="meta-item">
                        <strong>Longitude:</strong> {property.lon ?? 'N/A'}
                    </div>
                    <div className="meta-item">
                        <strong>Saved:</strong> {new Date(property.created_at).toLocaleDateString()}
                    </div>
                </div>
            </div>
        </div>
    )
}
