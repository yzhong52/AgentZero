import type { Property } from './types'
import type { ColDef } from './ListingTable'

/** Ranks each row's value for a column; only meaningful when every row has a value and they're not all tied. */
function rankCells(col: ColDef, rows: Property[]): ('winner' | 'loser' | null)[] {
  if (!col.compare) return rows.map(() => null)
  const values = rows.map(col.compare!.value)
  if (values.some(v => v === null) || new Set(values).size < 2) return rows.map(() => null)
  const nums = values as number[]
  const best = col.compare.direction === 'lower' ? Math.min(...nums) : Math.max(...nums)
  const worst = col.compare.direction === 'lower' ? Math.max(...nums) : Math.min(...nums)
  return nums.map(v => v === best ? 'winner' : v === worst ? 'loser' : null)
}

export function ListingComparison({ rows, cols, onRemove }: { rows: Property[]; cols: ColDef[]; onRemove: (id: number) => void }) {
  return (
    <div className="comparison-wrap">
      <table className="comparison-table">
        <thead>
          <tr>
            <th className="comparison-row-label" />
            {rows.map(p => {
              const img = p.images[0]?.url
              return (
                <th key={p.id} className="comparison-col-header">
                  <button
                    className="comparison-remove"
                    onClick={() => onRemove(p.id)}
                    aria-label="Remove from comparison"
                    type="button"
                  >
                    ×
                  </button>
                  <a className="comparison-thumb-btn" href={`/property/${p.id}`} target="_blank" rel="noopener noreferrer">
                    {img
                      ? <img src={img} alt={p.title} className="comparison-thumb" />
                      : <div className="comparison-thumb-placeholder" />
                    }
                    <span className="comparison-title">{p.title}</span>
                  </a>
                </th>
              )
            })}
          </tr>
        </thead>
        <tbody>
          {cols.map(c => {
            const ranks = rankCells(c, rows)
            return (
              <tr key={c.key}>
                <th className="comparison-row-label">{c.label}</th>
                {rows.map((p, i) => (
                  <td key={p.id} className={ranks[i] ? `comparison-${ranks[i]}` : undefined}>{c.render(p)}</td>
                ))}
              </tr>
            )
          })}
        </tbody>
      </table>
    </div>
  )
}
