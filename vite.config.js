import react from "@vitejs/plugin-react";
// só o painel passa pelo build; o resto de web/ continua servido como está (o MapLibre acha o worker pelo próprio caminho)
export default { plugins: [react()], resolve: { alias: { "@": new URL("./painel", import.meta.url).pathname } }, build: { outDir: "web/painel", emptyOutDir: true, cssCodeSplit: false, rollupOptions: { input: "painel/main.tsx", output: { entryFileNames: "painel.js", chunkFileNames: "[name].js", assetFileNames: "painel[extname]" } } } };
