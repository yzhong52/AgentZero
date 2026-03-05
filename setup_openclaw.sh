#!/bin/bash
# setup_openclaw.sh — Register AgentZero with OpenClaw
# Run this once after cloning the repo to set up the hourly listing ingestion cron job.
# Usage: ./setup_openclaw.sh

set -e

AGENT_ZERO_PATH="$(cd "$(dirname "$0")" && pwd)"
CRON_FILE="$HOME/.openclaw/cron/jobs.json"
TOOLS_FILE="$HOME/.openclaw/workspace/TOOLS.md"
JOB_ID="agent-zero-listing-ingest"

echo "🦞 Setting up AgentZero with OpenClaw..."
echo "   Repo path: $AGENT_ZERO_PATH"

# ── 1. Register hourly cron job ─────────────────────────────────────────────

mkdir -p "$(dirname "$CRON_FILE")"
[ -f "$CRON_FILE" ] || echo "[]" > "$CRON_FILE"

# Remove existing job (upsert)
python3 -c "
import json
jobs = json.load(open('$CRON_FILE'))
jobs = [j for j in jobs if j.get('id') != '$JOB_ID']
json.dump(jobs, open('$CRON_FILE', 'w'), indent=2)
"

# Add updated job
python3 -c "
import json
jobs = json.load(open('$CRON_FILE'))
jobs.append({
  'id': '$JOB_ID',
  'schedule': '0 * * * *',
  'description': 'Check real estate emails and ingest new listings into AgentZero',
  'task': 'Use the agent-zero skill to check Gmail for new real estate alert emails (Redfin, Zillow, REALTOR.ca, REW, etc.). AgentZero is at $AGENT_ZERO_PATH — start the backend if not running. For each email: open it in the browser, click the primary listing image to get the real URL, match the email to a search profile, then POST to http://localhost:8000/api/listings/suggest. Skip duplicates silently. Notify Yz on Slack only if new listings were added.',
  'model': 'github-copilot/claude-sonnet-4.6'
})
json.dump(jobs, open('$CRON_FILE', 'w'), indent=2)
print('✅ Cron job registered: $JOB_ID (hourly)')
"

# ── 2. Add TOOLS.md entry ────────────────────────────────────────────────────

if [ -f "$TOOLS_FILE" ]; then
  if grep -q "AgentZero" "$TOOLS_FILE"; then
    echo "✅ TOOLS.md already has an AgentZero entry — skipping"
  else
    cat >> "$TOOLS_FILE" << EOF

## AgentZero

- **Path:** \`AGENT_ZERO_PATH=$AGENT_ZERO_PATH\`
- **Backend:** http://localhost:8000 (Rust/Axum)
- **Frontend:** http://localhost:5173 (Vite/TypeScript)
- **Start:** \`./scripts/run_backend.sh\` and \`./scripts/run_frontend.sh\`
- **Logs:** \`/tmp/agent_zero_backend.log\`, \`/tmp/agent_zero_frontend.log\`
- **DB:** \`backend/listings.db\`
- **Skill:** \`agent-zero\` (handles start, listing ingest, email processing)
EOF
    echo "✅ TOOLS.md entry added"
  fi
else
  echo "⚠️  TOOLS.md not found at $TOOLS_FILE — skipping"
fi

echo ""
echo "🎉 Done! AgentZero will ingest real estate emails into OpenClaw every hour."
