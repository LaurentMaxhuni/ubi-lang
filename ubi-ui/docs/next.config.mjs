export default phase => ({
  output: "export",
  trailingSlash: true,
  distDir: phase === "phase-development-server" ? ".next-dev" : ".next",
  agentRules: false,
  images: { unoptimized: true },
});
