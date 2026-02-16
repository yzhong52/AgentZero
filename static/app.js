const form = document.getElementById("form");
const urlInput = document.getElementById("url");
const submitBtn = document.getElementById("submit");
const messageEl = document.getElementById("message");
const resultEl = document.getElementById("result");
const resultBody = document.getElementById("result-body");

function showMessage(text, type) {
  messageEl.textContent = text;
  messageEl.className = `message ${type}`;
  messageEl.hidden = false;
}

function hideMessage() {
  messageEl.hidden = true;
}

function renderValue(val) {
  if (val === undefined || val === null || String(val).trim() === "") {
    return null;
  }
  return String(val).trim();
}

function escapeHtml(s) {
  const div = document.createElement("div");
  div.textContent = s;
  return div.innerHTML;
}

function renderResult(data) {
  const fields = [
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

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const url = urlInput.value.trim();
  if (!url) return;

  hideMessage();
  resultEl.hidden = true;
  submitBtn.disabled = true;

  try {
    const resp = await fetch(`/api/parse?url=${encodeURIComponent(url)}`);
    const data = await resp.json().catch(() => null);

    if (!resp.ok) {
      const error = data;
      showMessage(error?.detail || "Request failed", "error");
      return;
    }

    renderResult(data);
    showMessage("Done. Parsed data shown below.", "success");
  } catch (err) {
    const error = err;
    showMessage(`Network error: ${error.message || "unknown"}`, "error");
  } finally {
    submitBtn.disabled = false;
  }
});
