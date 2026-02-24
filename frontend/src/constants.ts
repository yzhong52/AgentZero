export const STATUS_OPTIONS = ['Buyable', 'Interested', 'Pass'] as const
export type StatusOption = typeof STATUS_OPTIONS[number]

export const STATUS_COLORS: Record<string, string> = {
  Interested: '#4f46e5',
  Buyable: '#16a34a',
  Pass: '#9ca3af',
}
