# Plan: Automated Agent Review

Adds an AI triage loop so every newly added listing is automatically reviewed
by Claude — including profile assignment — before a human sees it.

## Workflow

```
User / cron submits URL only (no profile)
        ↓
Parse listing (Redfin, REW, Realtor, Zillow)
        ↓
Save to DB  →  status: PendingAgentReview,  search_profile_id: NULL
        ↓
Background task: call Claude API
  Input: structured Property JSON fields
       + ALL search profile titles + descriptions
        ↓
  ┌──────────────────────────────────────────────────────────┐
  │  status: PendingHumanReview                              │
  │  search_profile_id: 2                                    │
  │  comment: "3br under $1.8M in East Van, lane access"     │
  └──────────────────────────────────────────────────────────┘
        OR
  ┌──────────────────────────────────────────────────────────┐
  │  status: AgentSkip                                       │
  │  search_profile_id: null                                 │
  │  comment: "Burnaby — no profile targets this area"       │
  └──────────────────────────────────────────────────────────┘
```

Agent review uses **parsed property fields** (price, beds, baths, sqft,
location, schools, taxes, etc.) — not raw HTML — to keep the Claude call
small and fast.

---

## Status Set

| Status | Who sets it | Meaning |
|---|---|---|
| `PendingAgentReview` | System (on add) | Just added; agent has not reviewed yet |
| `PendingHumanReview` | Agent | Agent approved; assigned to a profile; awaiting human |
| `AgentSkip` | Agent | No profile matches; not worth reviewing |
| `Interested` | User | User is tracking this listing |
| `Buyable` | User | User considers this a strong candidate |
| `Pass` | User | User has dismissed this listing |

`PendingAgentReview` and `AgentSkip` are **agent-only** — not settable from
the UI status picker.

**Rename**: existing `Pending` → `PendingHumanReview` (data migration required).

---

## search_profile_id Becomes Nullable

Listings in `PendingAgentReview` state have no profile yet.
`search_profile_id` must become `Option<i64>` (nullable in DB and Rust/TS types).

---

## Implementation Checklist

### Step 1 — Backend: status rename + new statuses

- [ ] `backend/src/models/property.rs`
  - Rename `Pending` → `PendingHumanReview` in `ListingStatus` enum
  - Add `PendingAgentReview` and `AgentSkip` variants
  - Update `Display`, `FromStr`, sqlx impls for all three
  - Change `search_profile_id: i64` → `search_profile_id: Option<i64>` in `StoredProperty`

### Step 2 — Backend: DB migrations

- [ ] `0034_rename_pending_status.sql`
  ```sql
  UPDATE listings SET status = 'PendingHumanReview' WHERE status = 'Pending';
  ```

- [ ] `0035_agent_fields.sql`
  ```sql
  -- Add agent_comment column
  ALTER TABLE listings ADD COLUMN agent_comment TEXT;

  -- Make search_profile_id nullable:
  -- Drop the NOT NULL triggers added in 0031_rename_search_id.sql
  DROP TRIGGER IF EXISTS listings_search_criteria_id_notnull_insert;
  DROP TRIGGER IF EXISTS listings_search_criteria_id_notnull_update;
  ```

### Step 3 — Backend: StoredProperty + Property fields

- [ ] `agent_comment: Option<String>` added to `StoredProperty` and propagated through
  `Property::from_stored`, `row_to_property`, INSERT/UPDATE queries
- [ ] `search_profile_id: Option<i64>` change propagated through store queries,
  `From<Property> for StoredProperty`, `From<StoredProperty>` conversions

### Step 4 — Backend: `add.rs` — simplify suggest endpoint

- [ ] `SuggestBody` no longer requires `search_profile_id`
  - Old: `{ "url": "...", "search_profile_id": N }`
  - New: `{ "url": "..." }`
- [ ] Remove profile validation from `suggest` handler
- [ ] Initial status: `PendingAgentReview` (was `PendingHumanReview`)
- [ ] `search_profile_id` starts as `None`
- [ ] After saving, spawn background task: `tokio::spawn(run_agent_review(...))`
  - Returns immediately with `PendingAgentReview` property; review runs async

### Step 5 — Backend: new agent-review API endpoint

- [ ] `backend/src/api/agent_review.rs` (new file)
  - `POST /api/listings/:id/agent-review`
  - Body:
    ```json
    // approval
    { "status": "PendingHumanReview", "search_profile_id": 2, "comment": "..." }
    // skip
    { "status": "AgentSkip", "comment": "..." }
    ```
  - Validates:
    - `status` must be `PendingHumanReview` or `AgentSkip`
    - `search_profile_id` required when status is `PendingHumanReview`
    - `search_profile_id` must reference an existing profile
  - Updates `status`, `search_profile_id`, `agent_comment` atomically
  - Returns updated `Property`
- [ ] Wire route in `backend/src/main.rs`

### Step 6 — Backend: Claude API integration

- [ ] `backend/src/agent/mod.rs` (new module)
- [ ] `backend/src/agent/review.rs`
  ```
  run_agent_review(id: i64, property: StoredProperty, profiles: Vec<SearchProfile>, pool, client)
  ```
  - Builds compact prompt:
    ```
    You are reviewing a real estate listing to decide if it matches any search profile.

    Listing:
      Title: {title}
      Price: {price} {currency}
      Address: {street_address}, {city}
      Beds/Baths: {bedrooms} / {bathrooms}
      Sqft: {sqft}  Land: {land_sqft}
      Property tax: {property_tax}/yr   HOA: {hoa_monthly}/mo
      Monthly total: {monthly_total}
      Type: {property_type}   Built: {year_built}
      Schools: {elementary} ({rating}), {secondary} ({rating})
      Features: AC={ac}, radiant={radiant}, laundry={laundry}, garage={parking_garage}

    Search profiles:
      [1] {profile.title}: {profile.description}
      [2] {profile.title}: {profile.description}
      ...

    Pick the best matching profile, or skip if none fits.
    Reply with JSON only (no markdown):
    {"status":"PendingHumanReview","search_profile_id":N,"comment":"one sentence"}
    OR
    {"status":"AgentSkip","comment":"one sentence"}
    ```
  - HTTP POST to `https://api.anthropic.com/v1/messages`
    - Model: `claude-haiku-4-5-20251001` (fast + cheap for triage)
    - `max_tokens: 200`
  - Parses JSON response
  - Calls `POST /api/listings/{id}/agent-review` on localhost
  - Reads `ANTHROPIC_API_KEY` from environment
  - On any error: log warning, leave listing as `PendingAgentReview` (silent degradation)
- [ ] Declare `agent` module in `backend/src/lib.rs`

### Step 7 — Backend: `Cargo.toml`

- [ ] Confirm `serde_json` is available (needed for Claude API request/response body)

### Step 8 — Backend: TypeScript bindings

- [ ] `cargo test` — regenerates `frontend/src/bindings/ListingStatus.ts`

### Step 9 — Frontend: types + constants

- [ ] `frontend/src/constants.ts`
  - Add `PendingAgentReview`, `PendingHumanReview`, `AgentSkip` to `STATUS_OPTIONS`
  - Remove old `Pending`
  - Display labels:
    - `PendingHumanReview` → `"Review"`
    - `PendingAgentReview` → `"Analyzing…"`
    - `AgentSkip` → `"Skipped"`
  - Colors:
    - `PendingHumanReview` → amber (same as old `Pending`)
    - `PendingAgentReview` → muted blue-gray
    - `AgentSkip` → light gray
  - Add `AGENT_ONLY_STATUSES = ['PendingAgentReview', 'AgentSkip']` constant
    — excluded from the manual status picker
- [ ] `frontend/src/types.ts`
  - Replace `Pending` with `PendingHumanReview` in status union
  - Add `PendingAgentReview`, `AgentSkip`
  - `search_profile_id: number | null` (was `number`)
  - Add `agent_comment: string | null`

### Step 10 — Frontend: UI

- [ ] Status pill: render `PendingAgentReview` and `AgentSkip` with distinct styling
- [ ] Status picker: filter out `AGENT_ONLY_STATUSES` from selectable options
- [ ] Property detail view: show `agent_comment` as a subtle callout when non-null
  (label: "Agent note", small, muted, italic)
- [ ] Handle `search_profile_id: null` gracefully wherever profile is displayed
  (e.g. show "—" or "Unassigned" instead of crashing)

### Step 11 — CLI: `refresh_all.rs`

- [ ] Add color/label for `PendingAgentReview` (dim cyan, label "Analyzing…")
- [ ] Add color/label for `AgentSkip` (dim, label "Skipped")
- [ ] Update any reference to old `Pending` / `PENDING_STATUS` constant

### Step 12 — SKILL.md

- [ ] **Description**: update to say all sources supported (Safari/Chrome fallback); agent
  auto-assigns search profile
- [ ] **Key APIs table**:
  - Update `suggest` row: body is now `{ "url": "..." }` only
  - Add `POST /api/listings/:id/agent-review` row
- [ ] **Add Listing workflow**: simplify — no manual profile selection step;
  submit URL, agent handles profile assignment; listing arrives as `PendingAgentReview`
- [ ] **New section: Workflow: Agent Review** (standalone / manual re-run):
  - GET `/api/listings?status=PendingAgentReview`
  - GET `/api/search-profiles`
  - For each listing: compare fields against all profiles, decide
  - POST `/api/listings/:id/agent-review`
  - Log each decision
- [ ] **Daily Email Scan**: update log format — status after add is `PendingAgentReview`;
  note that agent review runs in background automatically
- [ ] **Key Property Fields**: add `agent_comment`, note `search_profile_id` is nullable
- [ ] **Logging format**: add agent review log entry examples

### Step 13 — Tests

- [ ] Unit test: prompt construction in `run_agent_review`
- [ ] API test: `POST /api/listings/:id/agent-review` rejects invalid status values
- [ ] API test: `PendingHumanReview` without `search_profile_id` is rejected
- [ ] Update snapshots referencing old `Pending` status

---

## Environment Variables

| Var | Required | Description |
|---|---|---|
| `ANTHROPIC_API_KEY` | Yes (for agent review) | Claude API key for triage calls |
| `SKIP_SAFARI` | No | `1` to bypass Safari, go straight to Chrome CDP |
| `BACKEND_PORT` | No | Backend port (default: 8000) |

---

## Deferred / Out of Scope

- Manual re-trigger of agent review on an existing listing (can add later)
- Retry logic for failed agent reviews (currently: stays as `PendingAgentReview` on error)
- Agent review for listings added via the non-suggest (manual URL) add endpoint
