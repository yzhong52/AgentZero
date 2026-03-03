//! Mortgage and monthly cost calculations.

use crate::models::property::Property;

/// Sums mortgage + monthly property tax + HOA into a total monthly cost.
pub(crate) fn compute_monthly_total(
    mortgage_monthly: Option<i64>,
    property_tax_annual: Option<i64>,
    hoa_monthly: Option<i64>,
) -> Option<i64> {
    let mortgage_monthly = mortgage_monthly?; // require at least a mortgage payment
    let property_tax_monthly = property_tax_annual.map(|t| t / 12).unwrap_or(0);
    let hoa_monthly = hoa_monthly.unwrap_or(0);
    Some(mortgage_monthly + property_tax_monthly + hoa_monthly)
}

/// Initial monthly mortgage interest only (principal * annual_rate / 12).
pub(crate) fn compute_initial_monthly_interest(price: i64, down_pct: f64, annual_rate: f64) -> i64 {
    let loan = price as f64 * (1.0 - down_pct);
    if loan <= 0.0 {
        return 0;
    }
    ((loan * annual_rate) / 12.0).round() as i64
}

/// Sums initial monthly interest + monthly property tax + HOA.
pub(crate) fn compute_monthly_cost(
    initial_monthly_interest: Option<i64>,
    property_tax_annual: Option<i64>,
    hoa_monthly: Option<i64>,
) -> Option<i64> {
    let initial_monthly_interest = initial_monthly_interest?;
    let property_tax_monthly = property_tax_annual.map(|t| t / 12).unwrap_or(0);
    let hoa_monthly = hoa_monthly.unwrap_or(0);
    Some(initial_monthly_interest + property_tax_monthly + hoa_monthly)
}

/// Standard amortisation formula: monthly payment on a fixed-rate mortgage.
/// Returns 0 if price is 0 or rate is 0 (handled gracefully).
pub(crate) fn compute_mortgage(price: i64, down_pct: f64, annual_rate: f64, years: i64) -> i64 {
    let loan = price as f64 * (1.0 - down_pct);
    if loan <= 0.0 {
        return 0;
    }
    let n = (years * 12) as f64;
    if annual_rate == 0.0 {
        return (loan / n).round() as i64;
    }
    let r = annual_rate / 12.0;
    let payment = loan * r * (1.0 + r).powf(n) / ((1.0 + r).powf(n) - 1.0);
    payment.round() as i64
}

pub(crate) fn recompute_with_stored_terms(target: &mut Property, stored: &Property) {
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years = stored.amortization_years.unwrap_or(25);

    target.down_payment_pct = Some(down_pct);
    target.mortgage_interest_rate = Some(rate);
    target.amortization_years = Some(years);

    recompute_from_explicit_terms(target, down_pct, rate, years);
}

pub(crate) fn recompute_from_explicit_terms(
    target: &mut Property,
    down_pct: f64,
    rate: f64,
    years: i64,
) {
    if let Some(price) = target.offer_price.or(target.price) {
        target.mortgage_monthly = Some(compute_mortgage(price, down_pct, rate, years));
    }

    target.monthly_total = compute_monthly_total(
        target.mortgage_monthly,
        target.property_tax,
        target.hoa_monthly,
    );

    let initial_interest = target
        .offer_price
        .or(target.price)
        .map(|price| compute_initial_monthly_interest(price, down_pct, rate));

    target.monthly_cost =
        compute_monthly_cost(initial_interest, target.property_tax, target.hoa_monthly);
}
