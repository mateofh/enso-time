const PORT_START = 5610;
const PORT_END = 5699;
const DEFAULTS = { enabled: true, port: null, token: null, clientId: crypto.randomUUID(), allowedSites: [] };

async function settings() {
  return { ...DEFAULTS, ...(await chrome.storage.local.get(DEFAULTS)) };
}

function baseUrl(port) {
  return `http://127.0.0.1:${port}/connectors/v1`;
}

async function findServer(preferredPort) {
  const ports = preferredPort ? [preferredPort] : Array.from({ length: PORT_END - PORT_START + 1 }, (_, index) => PORT_START + index);
  for (const port of ports) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/api/0/info`, { signal: AbortSignal.timeout(300) });
      if (response.ok) return port;
    } catch { /* Another local port or Ensō is closed. */ }
  }
  throw new Error("No se encontró Ensō Time entre los puertos 5610–5699.");
}

async function pair(code, preferredPort) {
  const current = await settings();
  const port = await findServer(preferredPort || current.port);
  const response = await fetch(`${baseUrl(port)}/pair`, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ code: String(code).trim() }),
  });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || `Emparejamiento rechazado (${response.status}).`);
  await chrome.storage.local.set({ port, token: result.token, clientId: result.clientId, enabled: true });
  return { port, clientId: result.clientId };
}

function normalizedHost(url) {
  try { return new URL(url).hostname.toLowerCase(); } catch { return null; }
}

function siteAllowed(host, allowedSites) {
  return Boolean(host && allowedSites.some((site) => host === site || host.endsWith(`.${site}`)));
}

async function visibleText(tabId) {
  const [{ result = "" } = {}] = await chrome.scripting.executeScript({
    target: { tabId },
    func: () => (document.body?.innerText || "").replace(/\s+/g, " ").trim().slice(0, 4000),
  });
  return result;
}

async function observe() {
  const current = await settings();
  if (!current.enabled || !current.token || !current.port) return;
  const [tab] = await chrome.tabs.query({ active: true, lastFocusedWindow: true });
  if (!tab?.id || !tab.url || tab.incognito || !/^https?:/.test(tab.url)) return;
  const domain = normalizedHost(tab.url);
  if (!domain) return;
  let textExcerpt;
  if (siteAllowed(domain, current.allowedSites)) {
    try { textExcerpt = await visibleText(tab.id); } catch { /* Site permission was revoked. */ }
  }
  const now = new Date().toISOString();
  const observation = {
    schemaVersion: 2,
    id: `${current.clientId}-${Date.now()}`,
    source: "browser",
    observedAt: now,
    durationSeconds: 0,
    app: "Chromium",
    title: String(tab.title || "").slice(0, 500) || null,
    domain,
    project: null,
    repository: null,
    branch: null,
    fileType: null,
    textExcerpt: textExcerpt || null,
    ocrTerms: [],
    sensitivity: "normal",
    clientId: current.clientId,
    createdAt: now,
  };
  const response = await fetch(`${baseUrl(current.port)}/observations`, {
    method: "POST",
    headers: { "Authorization": `Bearer ${current.token}`, "Content-Type": "application/json" },
    body: JSON.stringify(observation),
  });
  if (response.status === 401) await chrome.storage.local.set({ token: null });
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.alarms.create("enso-observe", { periodInMinutes: 1 });
  chrome.runtime.openOptionsPage();
});
chrome.action.onClicked.addListener(() => chrome.runtime.openOptionsPage());
chrome.alarms.onAlarm.addListener((alarm) => { if (alarm.name === "enso-observe") void observe(); });
chrome.tabs.onActivated.addListener(() => void observe());
chrome.tabs.onUpdated.addListener((_id, info) => { if (info.status === "complete") void observe(); });
chrome.runtime.onMessage.addListener((message, _sender, respond) => {
  if (message?.type === "pair") pair(message.code, message.port).then((value) => respond({ ok: true, value }), (error) => respond({ ok: false, error: error.message }));
  if (message?.type === "observe") observe().then(() => respond({ ok: true }), (error) => respond({ ok: false, error: error.message }));
  return true;
});
