import { useEffect, useState } from 'react'
import { useParams, useNavigate } from 'react-router-dom'
import type { Property } from './App'

function formatPrice(price: number | null, currency: string | null) {
    if (price == null) return null
    return new Intl.NumberFormat('en-CA', {
        style: 'currency',
        currency: currency ?? 'CAD',
        maximumFractionDigits: 0,
    }).format(price)
}

export function PropertyDetail() {
    const { id } = useParams<{ id: string }>()
    const navigate = useNavigate()
    const [property, setProperty] = useState<Property | null>(null)
    const [loading, setLoading] = useState(true)
    const [error, setError] = useState<string | null>(null)

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
            <button className="back-btn" onClick={() => navigate('/')}>
                ← Back
            </button>

            <div className="detail-images">
                {property.images.length > 0 ? (
                    <div className="image-carousel">
                        <img src={property.images[0]} alt={property.title} className="main-image" />
                        {property.images.length > 1 && (
                            <div className="image-thumbnails">
                                {property.images.map((img, idx) => (
                                    <img
                                        key={idx}
                                        src={img}
                                        alt={`${property.title} ${idx + 1}`}
                                        className="thumbnail"
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
