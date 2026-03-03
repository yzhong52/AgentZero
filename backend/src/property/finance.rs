use crate::models::property::Property;
use crate::{
    compute_initial_monthly_interest, compute_monthly_cost, compute_monthly_total,
    compute_mortgage,
};

pub fn recompute_with_stored_terms(target: &mut Property, stored: &Property) {
    let down_pct = stored.down_payment_pct.unwrap_or(0.20);
    let rate = stored.mortgage_interest_rate.unwrap_or(0.04);
    let years = stored.amortization_years.unwrap_or(25);

    target.down_payment_pct = Some(down_pct);
    target.mortgage_interest_rate = Some(rate);
    target.amortization_years = Some(years);

    recompute_from_explicit_terms(target, down_pct, rate, years);
}

pub fn recompute_from_explicit_terms(
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

    target.monthly_cost = compute_monthly_cost(initial_interest, target.property_tax, target.hoa_monthly);
}
