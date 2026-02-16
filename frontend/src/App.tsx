import { useState } from 'react'
import './App.css'

type ParseResult = {
  url: string
  title: string
  description: string
  images: string[]
  raw_json_ld: any[]
  meta: Record<string, string>
}

function App() {
  const [url, setUrl] = useState('')
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [result, setResult] = useState<ParseResult | null>(null)

  async function handleParse(e?: React.FormEvent) {
    e?.preventDefault()
    setError(null)
    setResult(null)
    if (!url) return setError('Please enter a URL')
    setLoading(true)
    try {
      const resp = await fetch(`/api/parse?url=${encodeURIComponent(url)}`)
      if (!resp.ok) throw new Error(await resp.text())
      const data: ParseResult = await resp.json()
      setResult(data)
    } catch (err: any) {
      setError(err?.message || String(err))
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="app-root">
      <h1>Property Parser</h1>
      <form onSubmit={handleParse} className="form-wrap">
        <div className="input-row">
          <input
            type="url"
            placeholder="https://example.com/listing"
            value={url}
            onChange={(e) => setUrl(e.target.value)}
          />
          <button type="submit" disabled={loading}>
            {loading ? 'Parsing…' : 'Parse'}
          </button>
        </div>
      </form>
      {error && <div className="message error">{error}</div>}
      {result && (
        <div className="result">
          <div style={{ padding: 12 }}>
            <h2>Result</h2>
            <p><strong>URL:</strong> {result.url}</p>
            <p><strong>Title:</strong> {result.title}</p>
            <p><strong>Description:</strong> {result.description}</p>
            <p><strong>Images:</strong></p>
            <ul>
              {result.images.map((src) => (
                <li key={src}><a href={src} target="_blank" rel="noreferrer">{src}</a></li>
              ))}
            </ul>
            <p><strong>Raw JSON-LD:</strong></p>
            <pre style={{ maxHeight: 300, overflow: 'auto' }}>{JSON.stringify(result.raw_json_ld, null, 2)}</pre>
          </div>
        </div>
      )}
    </div>
  )
}

export default App
