const backendProtocol = import.meta.env.VITE_BACKEND_PROTOCOL || window.location.protocol.replace(":", "");
const backendHost = import.meta.env.VITE_BACKEND_HOST || window.location.hostname;
const backendPort = import.meta.env.VITE_BACKEND_PORT || "3001";

const normalizedProtocol = backendProtocol.replace(/:$/, "");

export const backendHttpBaseUrl = `${normalizedProtocol}://${backendHost}:${backendPort}`;
export const backendApiBaseUrl = `${backendHttpBaseUrl}/api`;
export const backendWsUrl = `${normalizedProtocol === "https" ? "wss" : "ws"}://${backendHost}:${backendPort}/ws`;
