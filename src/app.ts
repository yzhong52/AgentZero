interface PropertyData {
  url?: string;
  address?: string;
  price?: string;
  beds?: string;
  baths?: string;
  sqft?: string;
  description?: string;
  title?: string;
  images?: string[];
  meta?: Record<string, string>;
}

interface ErrorResponse {
  detail: string;
}

const form = document.getElementById("form") as HTMLFormElement;
const urlInput = document.getElementById("url") as HTMLInputElement;
const submitBtn = document.getElementById("submit") as HTMLButtonElement;
const messageEl = document.getElementById("message") as HTMLDivElement;
const resultEl = document.getElementById("result") as HTMLDivElement;
const resultBody = document.getElementById("result-body") as HTMLDivElement;

function showMessage(text: string, type: "error" | "success"): void {
  messageEl.textContent = text;
  messageEl.className = `message ${type}`;
  messageEl.hidden = false;
}

function hideMessage(): void {
  messageEl.hidden = true;
}

function renderValue(val: unknown): string | null {
  if (val === undefined || val === null || String(val).trim() === "") {
    return null;
  }
  return String(val).trim();
}

function escapeHtml(s: string): string {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

function renderResult(data: PropertyData): void {
  const fields: Array<{ key: keyof PropertyData; label: string }> = [
    { key: "title", label: "Title" },
    { key: "address", label: "Address" },
    { key: "price", label: "Price" },
    { key: "beds", label: "Beds" },
    { key: "baths", label: "Baths" },
    { key: "sqft", label: "Sq ft" },
    { key: "description", label: "Description" },
  ];

  let html = "";

  if (data.url) {
    html += `<div class="field"><span class="field-label">Source</span><span class="field-value source-url"><a href="${escapeHtml(data.url)}" target="_blank" rel="noopener">${escapeHtml(data.url)}</a></span></div>`;
  }

  for (const f of fields) {
    const v = renderValue(data[f.key]);
    html += `<div class="field"><span class="field-label">${escapeHtml(f.label)}</span><span class="field-value${v ? "" : " empty"}">${v ? escapeHtml(v) : "—"}</span></div>`;
  }

  if (data.images && data.images.length > 0) {
    html += '<div class="field"><span class="field-label">Images</span><div class="field-value"><div class="images">';
    data.images.slice(0, 10).forEach((src) => {
      html += `<img src="${escapeHtml(src)}" alt="" loading="lazy" onerror="this.style.display='none'" />`;
    });
    html += "</div></div></div>";
  }

  resultBody.innerHTML = html;
  resultEl.hidden = false;
}

form.addEventListener("submit", async (e: SubmitEvent) => {
  e.preventDefault();
  const url = urlInput.value.trim();
  if (!url) return;

  hideMessage();
  resultEl.hidden = true;
  submitBtn.disabled = true;

  try {
    const resp = await fetch(`/api/parse?url=${encodeURIComponent(url)}`);
    const data: PropertyData | ErrorResponse = await resp.json().catch(() => null);

    if (!resp.ok) {
      const error = data as ErrorResponse;
      showMessage(error?.detail || "Request failed", "error");
      return;
    }

    renderResult(data as PropertyData);
    showMessage("Done. Parsed data shown below.", "success");
  } catch (err) {
    const error = err as Error;
    showMessage(`Network error: ${error.message || "unknown"}`, "error");
  } finally {
    submitBtn.disabled = false;
  }
});
