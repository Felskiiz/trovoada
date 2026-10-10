# Regras de design

## Referência
- Windy e Zoom Earth: mapa em tela cheia, controles flutuando por cima, legenda em barra de gradiente, balão da hora sobre o cursor do trilho, leitor de valores sob o mouse.
- O meteograma do painel segue o meteoblue: faixa por dia, barras de chuva na mesma rampa do mapa e curva de temperatura.

## Cor
- Toda cor de interface vem da paleta preto e branco de `cores_v` em `src/main.rs` (`api/cores.json` → variáveis CSS `--breu`, `--terra`, `--barro`, `--limo`, `--musgo`, `--acento`, `--ocre`, `--ferro`, `--areia`, `--fonte`). Não introduza cor nova; misture as que existem (`mix` em `camadas.js`).
- A paleta é recarregada a cada minuto (`cores()` + `tinta()`), então nada pode guardar cor fixa em CSS ou em canvas sem repintar. Antes ela vinha viva da petricor; desde 10/10/2026 é fixa em preto e branco, igual no local e no site público.
- O painel da cidade tem a altura do conteúdo (sem `bottom`), com `max-height` da janela e rolagem sem barra.
- Terra e oceano separados por mistura (`hx` em `app.js`): países `limo→fonte` 22%, Brasil 40%, oceano `--breu`.
- Temperatura (capitais e número grande do painel) usa a cor da faixa (`tcor`): <14 areia, <22 musgo, <28 acento, <33 ocre, acima ferro.
- Exceção: as legendas de radar e satélite (`LEG.radar`, `LEG.ir`) usam as cores reais dos ladrilhos de terceiros, amostradas dos PNG. Se trocar o esquema de cor no `obs.rs`, reamostre (ler o PNG e contar as cores) e atualize as paradas.
- Nível de alerta INMET: a cor do INMET só decide o nível (0 amarelo, 1 laranja, 2 vermelho); a cor desenhada é `ocre` (0) ou `ferro` (1 e 2).

## Forma
- Tudo é pill ou caixa de canto arredondado: chips com `border-radius:999px`, painéis com 22px, balões com 8 a 10px.
- Nunca use faixa lateral colorida (barra de acento) para marcar item selecionado: o selecionado muda o fundo inteiro (`aria-pressed=true` → fundo `--acento`, texto `--breu`).
- Profundidade por tom (`--terra` sobre o mapa, `--barro` nos botões) mais uma sombra suave comum (`--sombra`) em tudo que flutua sobre o mapa; nada de transparência em superfície.
- Botão principal (`#play`), cursor do trilho e balão da hora em `--acento`, com halo `--anel` (acento a 30%) no hover e quando ativo.
- Os chips de camada ficam juntos numa pill `--terra` só, cada um com glifo; ligado é fundo `--acento` e glifo `--breu`.
- Entrada: controles surgem com `surge` (fade + 10px) escalonado; o ponto do lugar pulsa. `prefers-reduced-motion` desliga as animações.
- Fontes: `var(--texto)` (Michelangelus, da Microsoft) para rótulos, `var(--mono)` para números e horas. Ícones são glifos Nerd Font `nf-md` (plano 15, `\u{F....}`); os de outros planos somem.
- Cursor do mapa é a seta normal: nada de mãozinha de agarrar (`.maplibregl-canvas-container` com `cursor:default`).

- O painel React usa os componentes Arc (`painel/components/arc/`, instalados pelo MCP `arc`, traduzidos para pt-BR). Os tokens do Arc (`--foreground`, `--surface`, `--accent`...) são remapeados para a paleta em `painel/estilo.css`; a cor de cada gráfico vem de `--accent` no invólucro (`.serie.mm` musgo, chance acento, `.serie.temp-h` ocre).
- Modo chuva: UF em `limo→acento` pela chance; pills `--terra` com UF em areia, temperatura na cor da faixa e chance em acento. Busca escurece o mundo (`cidade-m`, breu) e contorna o município em acento.
## Disposição
- Desktop: chips no topo à esquerda, botão `#recolhe` no topo à direita, painel da cidade à direita (`--pw`), legendas empilhadas sob o botão, linha do tempo no rodapé.
- Celular (≤900px): painel vira gaveta inferior (`--ph`, 62dvh), chips numa fileira com rolagem horizontal, legenda dentro do painel embaixo dos gráficos (`#leg-painel`) e, no mapa, só a pill `#dica` que abre o painel, trilho e polegar maiores para o dedo.
- O painel da cidade sempre abre recolhido (`body.sem`); recolhido, some inteiro, sem deixar resto.
- Gaveta do celular recolhida é só puxador e busca: `#lugar`, `#leg-painel` e `#rodape` ficam com opacidade 0 (senão vazam na faixa da safe area). Ao abrir, a gaveta corre na curva `--gaveta` (a da folha do iOS, .6s) e o conteúdo sobe 24px com fade escalonado; ao fechar, ele some em .15s antes de a gaveta descer. Arrastando, `--f` acompanha o dedo.
- Uma legenda por camada ligada, empilhadas em `#legenda` (`.item+.item`), na ordem em que foram ligadas.
- Pills que seguem o mouse (`#mira` com valores, `#aviso` com o alerta) ficam a 14px do cursor e têm `pointer-events:none`.

## Texto
- Português, curto, frase literal. Legenda explica em uma frase o que a cor quer dizer para quem não é meteorologista ("cinza é nuvem comum, colorido é nuvem alta que pode chover").
- Números em `pt-BR` (`num()`), vírgula decimal, hora como `ter 15h30`.

## Forma do código
- JS e CSS sem recuo e sem linha em branco; uma função por linha quando cabe. Comentário de uma linha acima do que não é óbvio, dizendo o porquê.
