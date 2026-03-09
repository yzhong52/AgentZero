export const STATUS_OPTIONS = ['Buyable', 'Interested', 'Pass', 'Pending'] as const
export type StatusOption = typeof STATUS_OPTIONS[number]
export const PENDING_STATUS: StatusOption = 'Pending'

// human‑readable labels shown in the UI; keeps the underlying value
// (`Buyable`) unchanged so the backend/data is unaffected.
export const STATUS_DISPLAY: Record<StatusOption, string> = {
  Buyable: 'Candidate',
  Interested: 'Interested',
  Pass: 'Pass',
  Pending: 'Pending',
}

export const STATUS_COLORS: Record<string, string> = {
  Pending: '#d97706',
  Interested: '#0369a1',
  Buyable: '#16a34a',
  Pass: '#9ca3af',
}

/**
 * User‑facing text for a status value.  Use wherever a label is rendered.
 *
 * The argument is the raw value (from the API or a constant); this returns the
 * corresponding display string. If the value is unrecognized we just return it
 * verbatim to avoid breaking anything.
 */
export function displayStatus(s: string | null | undefined): string {
  if (s == null) return ''
  return STATUS_DISPLAY[s as StatusOption] ?? s
}
