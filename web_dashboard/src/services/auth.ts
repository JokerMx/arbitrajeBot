import axios from "axios";
import { backendApiBaseUrl } from "./backendConfig";

// Token de la API de administración (ADMIN_API_TOKEN del backend).
// No se incrusta en el bundle: se pide una vez y se guarda en este navegador.
const STORAGE_KEY = "adminApiToken";

function readToken(): string {
  try {
    return localStorage.getItem(STORAGE_KEY) || "";
  } catch {
    return "";
  }
}

export function setAdminToken(token: string) {
  try {
    localStorage.setItem(STORAGE_KEY, token.trim());
  } catch {
    /* almacenamiento no disponible */
  }
}

let prompting = false;

function askForToken() {
  if (prompting) return;
  prompting = true;
  const token = window.prompt("Token de administración (ADMIN_API_TOKEN):");
  prompting = false;
  if (token) {
    setAdminToken(token);
    window.location.reload();
  }
}

function isApiUrl(url: string): boolean {
  return url.startsWith(backendApiBaseUrl) || url.startsWith("/api");
}

/** Añade el token a todas las llamadas a la API (fetch y axios) y lo pide ante un 401. */
export function installAuth() {
  const originalFetch = window.fetch.bind(window);
  window.fetch = async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = typeof input === "string" ? input : input instanceof URL ? input.href : input.url;
    if (!isApiUrl(url)) return originalFetch(input, init);
    const headers = new Headers(init?.headers || (input instanceof Request ? input.headers : undefined));
    const token = readToken();
    if (token) headers.set("Authorization", `Bearer ${token}`);
    const response = await originalFetch(input, { ...init, headers });
    if (response.status === 401) askForToken();
    return response;
  };

  axios.interceptors.request.use((config) => {
    const token = readToken();
    if (token && config.url && isApiUrl(config.url)) {
      config.headers = config.headers || {};
      config.headers["Authorization"] = `Bearer ${token}`;
    }
    return config;
  });
  axios.interceptors.response.use(
    (r) => r,
    (error) => {
      if (error?.response?.status === 401) askForToken();
      return Promise.reject(error);
    }
  );
}
