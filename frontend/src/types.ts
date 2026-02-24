export type ImageEntry = {
  id: number
  url: string
  created_at: string
}

export type Property = {
  id: number
  redfin_url: string | null
  realtor_url: string | null
  rew_url: string | null
  zillow_url: string | null
  title: string
  description: string
  price: number | null
  price_currency: string | null
  offer_price: number | null
  street_address: string | null
  city: string | null
  region: string | null
  postal_code: string | null
  country: string | null
  bedrooms: number | null
  bathrooms: number | null
  sqft: number | null
  year_built: number | null
  lat: number | null
  lon: number | null
  images: ImageEntry[]
  created_at: string
  updated_at: string | null
  notes: string | null
  parking_garage: number | null
  total_parking_space: number | null
  parking_covered: number | null
  parking_open: number | null
  land_sqft: number | null
  property_tax: number | null
  skytrain_station: string | null
  skytrain_walk_min: number | null
  radiant_floor_heating: boolean | null
  ac: boolean | null
  down_payment_pct: number | null
  mortgage_interest_rate: number | null
  amortization_years: number | null
  mortgage_monthly: number | null
  hoa_monthly: number | null
  monthly_total: number | null
  monthly_cost: number | null
  has_rental_suite: boolean | null
  rental_income: number | null
  status: string
  school_elementary: string | null
  school_elementary_rating: number | null
  school_middle: string | null
  school_middle_rating: number | null
  school_secondary: string | null
  school_secondary_rating: number | null
}