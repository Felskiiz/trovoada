# Armadilhas e como verificar

## Armadilhas que já custaram tempo
- `cargo` não está no PATH: use `~/.cargo/bin/cargo`.
- Reiniciar o servidor logo depois de matar o antigo dá `AddrInUse`; espere o processo morrer (`pgrep -x trovoada`) e suba de novo.
- `updateImage({url})` numa fonte image do MapLibre pisca; use `_setImage(canvas)` (ver `logica.md`).
- Quadro de previsão que "some" no fim da linha do tempo era quadro com uma fonte só de domínio parcial; o filtro de 90% de cobertura em `consenso::tempos` resolve. Não o afrouxe.
- O satélite GIBS Clean IR não é cinza puro: topos frios vêm coloridos (azul → ciano → verde → amarelo → vermelho). Legenda feita de suposição sai errada; amostre os PNG.
- O radar RainViewer no esquema 2 não tem verde: bege fraco, azuis, amarelo, laranja, vermelho, rosa.
- Glifo Nerd Font desenha mais largo que o avanço da fonte e invade o texto ao lado: dê largura fixa ao `.g` (ex.: `width:17px`).
- Print do Firefox headless sai no meio da animação de entrada; o perfil de captura usa `ui.prefersReducedMotion=1`.
- Glifo de ícone fora do plano 15 (`nf-md`) some na fonte do site.
- O site no celular não abre se o servidor escuta em `127.0.0.1`.

## Verificação
- `node --check web/app.js` depois de cada edição no JS.
- `~/.cargo/bin/cargo test --release` para o servidor (os testes de rede ficam em `-- --ignored`).
- Visual: abra o site e olhe a coisa fazendo a coisa. Sem navegador, o Firefox headless tira print: `firefox --headless --screenshot saida.png --window-size 1200,800 http://127.0.0.1:8077/`. Ladrilhos de radar e satélite podem não terminar de carregar no print; buraco retangular no satélite costuma ser só isso (confira o tile com `curl` → 200).
- O site é estático em `web/` e o servidor o lê do disco: editar HTML, JS ou CSS não pede reinício, só recarregar a página. Mudança em `src/` pede `cargo build --release` e reinício.
