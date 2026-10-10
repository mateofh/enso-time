const vscode = require("vscode");

const PORT_START = 5610;
const PORT_END = 5699;
const SECRET_TOKEN = "ensoTime.connectorToken";
const STATE_PORT = "ensoTime.serverPort";
const STATE_CLIENT = "ensoTime.clientId";
let timer;

function baseUrl(port) { return `http://127.0.0.1:${port}/connectors/v1`; }

async function findServer(preferred) {
  const ports = preferred ? [preferred] : Array.from({ length: PORT_END - PORT_START + 1 }, (_, i) => PORT_START + i);
  for (const port of ports) {
    try {
      const response = await fetch(`http://127.0.0.1:${port}/api/0/info`, { signal: AbortSignal.timeout(300) });
      if (response.ok) return port;
    } catch { /* Ensō is closed or another service owns the port. */ }
  }
  throw new Error("No se encontró Ensō Time entre los puertos 5610–5699.");
}

async function pair(context) {
  const code = await vscode.window.showInputBox({ prompt: "Código de ocho dígitos mostrado por Ensō Time", password: true, validateInput: (value) => /^\d{8}$/.test(value) ? null : "Usa exactamente ocho dígitos" });
  if (!code) return;
  const port = await findServer(context.globalState.get(STATE_PORT));
  const response = await fetch(`${baseUrl(port)}/pair`, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ code }) });
  const result = await response.json();
  if (!response.ok) throw new Error(result.error || `Emparejamiento rechazado (${response.status}).`);
  await context.secrets.store(SECRET_TOKEN, result.token);
  await context.globalState.update(STATE_PORT, port);
  await context.globalState.update(STATE_CLIENT, result.clientId);
  vscode.window.showInformationMessage("Ensō Time quedó emparejado con este editor.");
  await observe(context);
}

function repositoryContext(document) {
  const git = vscode.extensions.getExtension("vscode.git")?.exports?.getAPI?.(1);
  const repository = git?.repositories?.find((repo) => document?.uri?.fsPath?.startsWith(repo.rootUri.fsPath));
  return {
    repository: repository?.rootUri?.path?.split("/").filter(Boolean).pop() || null,
    branch: repository?.state?.HEAD?.name || null,
  };
}

async function send(context, textExcerpt = null) {
  const token = await context.secrets.get(SECRET_TOKEN);
  const port = context.globalState.get(STATE_PORT);
  const clientId = context.globalState.get(STATE_CLIENT);
  if (!token || !port || !clientId) return;
  const editor = vscode.window.activeTextEditor;
  const document = editor?.document;
  const workspace = document ? vscode.workspace.getWorkspaceFolder(document.uri) : vscode.workspace.workspaceFolders?.[0];
  const repo = repositoryContext(document);
  const now = new Date().toISOString();
  const observation = {
    schemaVersion: 2,
    id: `${clientId}-${Date.now()}`,
    source: "editor",
    observedAt: now,
    durationSeconds: 0,
    app: vscode.env.appName,
    title: null,
    domain: null,
    project: workspace?.name || null,
    repository: repo.repository,
    branch: repo.branch,
    fileType: document?.languageId || null,
    textExcerpt: textExcerpt ? textExcerpt.slice(0, 8000) : null,
    ocrTerms: [],
    sensitivity: textExcerpt ? "manual" : "normal",
    clientId,
    createdAt: now,
  };
  const response = await fetch(`${baseUrl(port)}/observations`, { method: "POST", headers: { "Authorization": `Bearer ${token}`, "Content-Type": "application/json" }, body: JSON.stringify(observation) });
  if (response.status === 401) await context.secrets.delete(SECRET_TOKEN);
  if (!response.ok) throw new Error((await response.json().catch(() => ({}))).error || `Ensō rechazó el contexto (${response.status}).`);
}

async function observe(context) {
  try { await send(context); } catch (error) { console.warn("Ensō Time Context:", error.message); }
}

async function shareSelection(context) {
  const editor = vscode.window.activeTextEditor;
  const selected = editor?.document.getText(editor.selection).trim();
  if (!selected) return vscode.window.showWarningMessage("Selecciona primero el fragmento que quieres compartir.");
  const choice = await vscode.window.showWarningMessage(`Compartir una vez ${Math.min(selected.length, 8000)} caracteres con Ensō Time local.`, { modal: true }, "Compartir");
  if (choice !== "Compartir") return;
  await send(context, selected);
  vscode.window.showInformationMessage("El fragmento fue compartido localmente con Ensō Time.");
}

async function disconnect(context) {
  await context.secrets.delete(SECRET_TOKEN);
  await context.globalState.update(STATE_PORT, undefined);
  vscode.window.showInformationMessage("Este editor dejó de enviar contexto a Ensō Time.");
}

function activate(context) {
  context.subscriptions.push(
    vscode.commands.registerCommand("ensoTime.pair", () => pair(context).catch((error) => vscode.window.showErrorMessage(error.message))),
    vscode.commands.registerCommand("ensoTime.shareSelection", () => shareSelection(context).catch((error) => vscode.window.showErrorMessage(error.message))),
    vscode.commands.registerCommand("ensoTime.disconnect", () => disconnect(context)),
    vscode.window.onDidChangeActiveTextEditor(() => void observe(context)),
    vscode.workspace.onDidChangeWorkspaceFolders(() => void observe(context)),
  );
  timer = setInterval(() => void observe(context), 60_000);
  context.subscriptions.push({ dispose: () => clearInterval(timer) });
  void observe(context);
}

function deactivate() { if (timer) clearInterval(timer); }
module.exports = { activate, deactivate, findServer };
