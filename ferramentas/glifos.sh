#!/bin/sh
# Recorta a LXGWWenKaiMono Nerd Font para web/fontes/mono.woff2: latim, pontuação e só os glifos Nerd que o código usa.
# Rode de novo ao pôr glifo novo; sem isso ele some onde a fonte não está instalada (iPhone).
f=${1:-$HOME/.local/share/fonts/LXGWWenKaiMonoNerdFont-Regular.ttf}
g=$(grep -ohE '\\u\{F[0-9A-F]{4}\}|&#xF[0-9A-F]{4};' web/*.js web/index.html painel/*.tsx | grep -oE 'F[0-9A-F]{4}' | sort -u | sed 's/^/U+/' | paste -sd,)
pyftsubset "$f" --unicodes="U+0020-007E,U+00A0-017F,U+2010-2027,U+2190-2193,U+2212,U+00D7,$g" --flavor=woff2 --output-file=web/fontes/mono.woff2
