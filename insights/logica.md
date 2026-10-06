# Lógica do site e do servidor

A arquitetura (fontes, consenso, API) está no `README.md`. Aqui fica o comportamento que não se lê de lá.

## Linha do tempo (`web/app.js`: `linha`, `vai`, `marcas`, `toca`)
- Dois modos, em `S.modo`:
  - Previsão (`S.modo=false`): anda por `S.tempos` (quadros de 3 h vindos de `/api/tempos`), 12 passos de 15 min por quadro (`K()`). Entre dois quadros, `mistura()` interpola chuva, vento, temperatura etc. linearmente; frentes vêm do quadro mais próximo.
  - Observação (`S.modo=true`): ativo sempre que radar ou satélite está ligado. Anda pelos quadros reais das ~2 h passadas (`S.obs` = união dos instantes de radar e satélite das camadas ligadas, de 10 em 10 min). Cada camada mostra o seu quadro mais recente até o instante (`ate()`). Nesse modo as camadas de previsão, o vento, o leitor `#mira` e as temperaturas das capitais saem do mapa, porque são futuro e não combinam com o passado.
- Mudou o modo ou o conjunto de quadros (`S.chave`): o cursor volta ao fim (observação) ou ao agora (previsão).
- Animação: `toca(on)` com `setInterval` de `k1()/S.vel`; em 1×, 200 ms por passo de previsão e 1 s por quadro observado. `S.vel` ∈ {0,5, 1, 1,5, 2}, trocada pelo botão `#vel` sem parar a animação.
- O servidor só entrega quadros com 90% da grade coberta (`consenso::tempos`): no fim da janela resta uma fonte de domínio parcial e o quadro aparecia quase vazio.

## Pintura das camadas
- Camadas de previsão (chuva, massas, tempestade) são canvas por camada passados a `source._setImage(canvas)`. Não use `updateImage({url})`: ele recarrega a fonte e pisca um quadro vazio a cada passo.
- `pede()` agrupa repinturas num `requestAnimationFrame`; chame-o depois de mudar `S.campo` ou o modo.
- Vento são partículas num canvas próprio (`vento()` em `camadas.js`); `V.dados(null)` as para.
- Radar e satélite são fontes raster; trocar o instante é `setTiles` com a URL nova (`radarEm`, `irEm`), e os PNG passam pelo proxy com cache do servidor (`/tile/{camada}/{t}/...`). O instante do satélite na URL é ISO (`2026-10-05T23:30:00Z`); o do radar é Unix.
- `campo(t)` guarda até 16 quadros em `S.cache` e pré-busca os dois seguintes.

## Painel da cidade
- `lugar()` marca o ponto, preenche `#agora` com o quadro mais próximo de agora (não o do cursor) e busca `/api/ponto` para o meteograma (`horas()`) e a tabela por modelo (`tabela()`).
- Alertas do lugar: `blocoAlertas()` filtra por ponto dentro do polígono. Passar o mouse num alerta no mapa mostra `#aviso` com nome e severidade; clicar abre o painel no bloco de alertas.

## Servidor
- Escuta em `0.0.0.0:8077` para abrir no celular pela rede local ou Tailscale; o firewalld já libera TCP 1025–65535.
- Cada família de fontes tem o seu ciclo (`ciclo()` em `main.rs`); em erro mantém o dado anterior e marca o erro no status que o rodapé mostra.
- Raios do GLM só são buscados enquanto alguém pediu `/api/raios` nos últimos 5 min.
- Cache em disco: `~/.cache/trovoada` (Open-Meteo 8 h; tiles de instante apagados depois de 24 h).
