// DIM ingress reaches workspace ports, while Docker runs on agent-dind.
// A byte-preserving relay keeps Router4u's Host/Origin and streaming intact.
import { createServer, connect } from "node:net";

const server = createServer((client) => {
  const upstream = connect({ host: "agent-dind", port: 9470 });
  client.on("error", () => upstream.destroy());
  upstream.on("error", () => client.destroy());
  client.on("close", () => upstream.destroy());
  upstream.on("close", () => client.destroy());
  client.pipe(upstream).pipe(client);
});
server.listen(9470, "0.0.0.0");
for (const signal of ["SIGINT", "SIGTERM"]) {
  process.on(signal, () => server.close());
}
