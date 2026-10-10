# Lógica do site e do servidor

A arquitetura (fontes, consenso, API) está no `README.md`. Aqui fica o comportamento que não se lê de lá.

## Linha do tempo (`web/app.js`: `linha`, `vai`, `marcas`, `toca`)
- Dois modos, em `S.modo`:
  - Previsão (`S.modo=false`): anda por `S.tempos` (quadros de 3 h vindos de `api/tempos.json`), 12 passos de 15 min por quadro (`K()`). Entre dois quadros, `mistura()` interpola chuva, vento, temperatura etc. linearmente; frentes vêm do quadro mais próximo.
  - Observação (`S.modo=true`): ativo sempre que radar ou satélite está ligado. Anda pelos quadros reais das ~2 h passadas (`S.obs` = união dos instantes de radar e satélite das camadas ligadas, de 10 em 10 min). Cada camada mostra o seu quadro mais recente até o instante (`ate()`). Nesse modo as camadas de previsão, o vento, o leitor `#mira` e as temperaturas das capitais saem do mapa, porque são futuro e não combinam com o passado.
- Mudou o modo ou o conjunto de quadros (`S.chave`): o cursor volta ao fim (observação) ou ao agora (previsão).
- Animação: `toca(on)` com `setInterval` de `k1()/S.vel`; em 1×, 200 ms por passo de previsão e 1 s por quadro observado. `S.vel` ∈ {0,5, 1, 1,5, 2}, trocada pelo botão `#vel` sem parar a animação.
- O servidor só entrega quadros com 90% da grade coberta (`consenso::tempos`): no fim da janela resta uma fonte de domínio parcial e o quadro aparecia quase vazio.

## Pintura das camadas
- Camadas de previsão (chuva, massas, tempestade) são canvas por camada passados a `source._setImage(canvas)`. Não use `updateImage({url})`: ele recarrega a fonte e pisca um quadro vazio a cada passo.
- `pede()` agrupa repinturas num `requestAnimationFrame`; chame-o depois de mudar `S.campo` ou o modo.
- Vento são partículas num canvas próprio (`vento()` em `camadas.js`); `V.dados(null)` as para.
- Radar e satélite são fontes raster; trocar o instante é `setTiles` com a URL nova (`radarEm`, `irEm`), e os PNG vêm direto do RainViewer (`host` + `path` do `weather-maps.json`, buscado pelo navegador em `passado()`) e do GIBS. O instante do satélite na URL é ISO (`2026-10-05T23:30:00Z`); o do radar é Unix.
- `campo(t)` guarda até 16 quadros em `S.cache` e pré-busca os dois seguintes.

## Modos chuva e precipitação
- Ao abrir, só `chuva` vem ligado. `chuva` é o modo simples: `ufChuva()` pinta cada UF pela chance média (`chance()` = fontes que votam chuva ÷ fontes) e `placas()` desenha pills `UF temp chance` no canvas `#placas`. O antigo modo de mm/h agora é `precip` (fonte e camada `precip`).
- Cidade por cidade só quando a UF do centro cobre ≥55% da tela (`cheio` em `placas()`): até 600 municípios na ordem de `municipios.json`, que vem ordenado por população (estimativa IBGE, SIDRA 6579), com colisão gulosa e folga de 18 px no zoom 9 a 50 px no zoom 7: no zoom de estado só as maiores aparecem. Painel, chips, legenda, linha do tempo e o marcador entram como obstáculos.
- `#placas` mede `map.getContainer()`: o `getCanvasContainer()` tem 0 px.
- A cidade escolhida (busca, GPS ou clique) ganha a placa primeiro, 24 px acima do marcador, e sai do laço; `lugar()` chama `placas()`. A chance leva o ícone de nuvem com chuva (`\u{F0597}`). Clique destaca o contorno igual à busca, sem voo: `lugar(l,false)` só pula o `fitBounds`.
## Painel da cidade
- O painel é React (`painel/`, build do Vite em `web/painel/`); o mapa segue JS sem bundle. `painel()` no `app.js` monta o dado, grava em `window.trovoada.lugar` e dispara `trovoada:lugar`; `painel/lugar.tsx` lê com `useSyncExternalStore`.
- Dado: temperatura e vento do quadro de agora, mais as 24 h do dia local vindas de `ponto()` (Open-Meteo direto, `past_days=1` para o dia começar à meia-noite local): mm é a mediana dos modelos, chance é a fração de modelos com ≥0,1 mm, temperatura é a mediana.
- Busca: `lugar(l,true)` baixa a malha do IBGE (`l.ibge`), desenha no canvas `#mascara` (`mascara()`) o mundo em breu a 72% com o município de buraco e o contorno em acento e faz `fitBounds` descontando o painel. Clique no mapa só marca o ponto, sem zoom nem contorno.
- Ao abrir: `api/lugar.json` já entra com voo e contorno, sem esperar o GPS (que leva até 10 s e, em http fora do localhost, nem existe). Se a `geolocation` do navegador responder antes de outra escolha, `nomeado()` (município mais próximo, com `ibge`) → `lugar(l,true)` voa até ele.
- Carga: a pintura espera só `tempos()` + `dados/ufs.json` (que o app baixa e passa por `setData`, sem a fonte do MapLibre baixar de novo). Radar, satélite, alertas e municípios carregam por fora; antes, um `Promise.all` com eles segurava as cores por segundos.
- Limites de todos os municípios: `web/dados/municipios-geo.json` (IBGE `malhas/paises/BR?intrarregiao=municipio&qualidade=minima`, coordenadas a 3 casas, sem propriedades). A fonte `munis` nasce vazia e só recebe o arquivo no primeiro `zoomend` com zoom ≥ 5,5; a camada `munis-l` tem `minzoom:6` e aparece de 6 a 7.
## Servidor
- Escuta em `0.0.0.0:8077` para abrir no celular pela rede local ou Tailscale; o firewalld já libera TCP 1025–65535.
- Cada família de fontes tem o seu ciclo (`ciclo()` em `main.rs`); em erro mantém o dado anterior e marca o erro no status que o rodapé mostra.
- Raios do GLM só são buscados enquanto alguém pediu `/api/raios.json` nos últimos 5 min.
- Cache em disco: `~/.cache/trovoada` (Open-Meteo 8 h). `XDG_CACHE_HOME` muda a pasta; o Actions usa isso para guardar a grade entre rodadas.
- `--exportar <pasta>`: uma rodada de cada fonte e sai, gravando `<pasta>/api/` com os nomes das rotas. É o que o site público (GitHub Pages) serve.
