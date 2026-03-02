export const STATUS_OPTIONS = ['Buyable', 'Interested', 'Pass', 'Pending'] as const
export type StatusOption = typeof STATUS_OPTIONS[number]

export const STATUS_COLORS: Record<string, string> = {
  Pending: '#d97706',
  Interested: '#0369a1',
  Buyable: '#16a34a',
  Pass: '#9ca3af',
}
