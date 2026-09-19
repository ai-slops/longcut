import { fileURLToPath } from "node:url";
import { createGateway } from "./gateway.mjs";

const port = Number(process.env.PORT ?? "3000");
if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("PORT must be between 1 and 65535");
const server = createGateway({
  backend: process.env.LONGCUT_BACKEND ?? "http://agent-dind:4000",
  webRoot: fileURLToPath(new URL("../apps/web/dist/", import.meta.url)),
});
server.requestTimeout = 30 * 60 * 1000;
server.listen(port, process.env.LONGCUT_GATEWAY_BIND ?? "127.0.0.1", () => {
  process.stdout.write(JSON.stringify({ event: "gateway.listening", port }) + "\n");
});
for (const signal of ["SIGTERM", "SIGINT"]) {
  process.on(signal, () => {
    server.close();
    server.closeIdleConnections();
  });
}
