const $ = (id) => document.getElementById(id);
const status = (message, error = false) => { $("status").textContent = message; $("status").className = error ? "error" : "success"; };

async function load() {
  const stored = await chrome.storage.local.get({ enabled: true, port: null, token: null, allowedSites: [] });
  $("enabled").checked = stored.enabled;
  $("port").value = stored.port || "";
  $("sites").value = stored.allowedSites.join("\n");
  status(stored.token ? "Conector emparejado." : "Todavía no está emparejado.", !stored.token);
}

async function save() {
  const allowedSites = [...new Set($("sites").value.split(/\s+/).map((value) => value.trim().toLowerCase()).filter(Boolean))];
  const origins = allowedSites.flatMap((site) => [`https://${site}/*`, `https://*.${site}/*`, `http://${site}/*`, `http://*.${site}/*`]);
  if (origins.length) await chrome.permissions.request({ origins });
  await chrome.storage.local.set({ enabled: $("enabled").checked, allowedSites });
  status("Política guardada.");
}

$("pair").addEventListener("click", async () => {
  const code = $("code").value.trim();
  const port = Number($("port").value) || null;
  const response = await chrome.runtime.sendMessage({ type: "pair", code, port });
  if (!response?.ok) return status(response?.error || "No se pudo emparejar.", true);
  $("port").value = response.value.port;
  status("Emparejamiento completado.");
});
$("save").addEventListener("click", () => void save());
void load();
