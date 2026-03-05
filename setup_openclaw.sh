#!/bin/bash
# setup_openclaw.sh — Register AgentZero with OpenClaw
# Run this once after cloning the repo to set up the hourly listing ingestion cron job.
# Usage: ./setup_openclaw.sh

set -e

AGENT_ZERO_PATH="$(cd "$(dirname "$0")" && pwd)"
TOOLS_FILE="$HOME/.openclaw/workspace/TOOLS.md"
JOB_NAME="agent-zero-listing-ingest"
JOB_TASK="Check Gmail for new real estate alert emails and ingest new listings into AgentZero at $AGENT_ZERO_PATH.\n\
Start the backend if it is not already running.\n\
For each email:\n\
- Open it in the browser.\n\
- Click the primary listing image to get the real URL.\n\
- Match it to a search profile.\n\
- POST it to http://localhost:8000/api/listings/suggest.\n\
- Skip duplicates silently.\n\
- After processing, apply the Gmail label '_open_claw_agent_zero' to it using the Himalaya skill.\n\
At the end:\n\
- Send Yz a summary on Slack of what was processed (new listings added, duplicates skipped, emails labeled).\n\
- If nothing new, still send a brief summary."

echo "🦞 Setting up AgentZero with OpenClaw..."
echo "   Repo path: $AGENT_ZERO_PATH"

# ── 1. Register hourly cron job ─────────────────────────────────────────────

# Upsert: edit if exists, add if not
EXISTING_ID=$(openclaw cron list --json 2>/dev/null | python3 -c "
import json, sys
jobs = json.load(sys.stdin)
match = next((j for j in jobs if j.get('name') == '$JOB_NAME'), None)
print(match['id'] if match else '')
" 2>/dev/null || echo "")

if [ -n "$EXISTING_ID" ]; then
  openclaw cron edit "$EXISTING_ID" \
    --every "1h" \
    --system-event "$JOB_TASK" \
    --description "Check real estate emails and ingest new listings into AgentZero" \
    --model "github-copilot/claude-sonnet-4.6" \
    --session "main"
  echo "✅ Cron job updated: $JOB_NAME ($EXISTING_ID)"
else
  openclaw cron add \
    --name "$JOB_NAME" \
    --every "1h" \
    --system-event "$JOB_TASK" \
    --description "Check real estate emails and ingest new listings into AgentZero" \
    --model "github-copilot/claude-sonnet-4.6" \
    --session "main"
fi

echo "✅ Cron job registered: $JOB_NAME (every 1h)"

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
