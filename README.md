# trovoada

Radar de tempestades do Brasil, local. Servidor em Rust (axum) e mapa em MapLibre, sem tiles de terceiros: o mapa-base é GeoJSON em `web/dados`. As cores vêm da paleta viva da petricor, em `~/.config/quickshell/petricor/cores.json`.

## Rodar

```
cd ~/trovoada && ~/.cargo/bin/cargo run --release
```

Depois abra http://127.0.0.1:8077. O `~/.cargo/bin` não está no PATH.

Ao subir, o servidor baixa o GFS e o ECMWF em GRIB, o que leva cerca de 35 s. A primeira grade completa do Open-Meteo leva cerca de 10 min por causa do limite de taxa da API; depois disso fica em cache por 8 h em `~/.cache/trovoada/openmeteo.json`. Enquanto a grade não chega, o consenso usa só os modelos que já carregaram.

## Consenso

Grade de 0,5° (lat 14 a −58, lon −110 a 0: a América do Sul inteira e o Atlântico até a África, de onde chegam as frentes; fora da malha de 1° do Open-Meteo votam só ECMWF e GFS), quadros a cada 3 h de −72 h a +72 h, com o agora no meio da linha do tempo; o passado vem das rodadas anteriores de cada modelo. Votam cinco modelos:

- ECMWF IFS, peso 1,25, do GRIB aberto
- GFS, do filtro NOMADS
- ICON, UKMO e CMA, pelo Open-Meteo

Cada célula chove quando a maioria ponderada passa de 0,2 mm/h. Temperatura e vento são a média ponderada dos modelos. As frentes saem do gradiente de temperatura em 850 hPa, afinado por supressão de não-máximos. A advecção decide se a frente é fria ou quente.

## Observação

- Radar: RainViewer
- Satélite: GIBS, infravermelho e IMERG
- Raios: GOES GLM
- Avisos: INMET

## API

`/api/tempos`, `/api/campo`, `/api/ponto`, `/api/radar`, `/api/satelite`, `/api/alertas`, `/api/raios`, `/api/cores`, `/api/lugar` e `/tile/{camada}/{t}/{z}/{x}/{y}`.

## Testes

```
~/.cargo/bin/cargo test --release
```

Os testes ignorados batem na rede: `-- --ignored`.
