import { useSyncExternalStore } from "react";
import { BarChart } from "@/components/arc/bar-chart/bar-chart";
type Hora = { h: number; mm: number; chance: number; t: number };
type Dado = { nome: string; uf?: string; t: number; cor: string; vento: number | null; dir: number | null; dia: string; horas: Hora[] | null; media: number | null };
declare global { interface Window { trovoada?: { lugar?: Dado | null } } }
// o app.js (mapa, fora do React) publica o lugar em window.trovoada e avisa pelo evento
const assina = (f: () => void) => { addEventListener("trovoada:lugar", f); return () => removeEventListener("trovoada:lugar", f); };
const le = () => window.trovoada?.lugar ?? null;
const n1 = new Intl.NumberFormat("pt-BR", { maximumFractionDigits: 1 });
const grau = (v: number) => `${Math.round(v)}°`;
export function Lugar() {
const d = useSyncExternalStore(assina, le);
if (!d) return null;
const serie = (f: (x: Hora) => number) => (d.horas ?? []).map(x => ({ key: String(x.h), label: `${x.h}h`, axisLabel: x.h % 6 ? undefined : `${x.h}h`, value: f(x) }));
const period = `hoje, ${d.dia}`;
return <>
<section id="agora">
<div className="temp" style={{ color: d.cor }}>{grau(d.t)}</div>
<div className="info">
<div className="nome">{d.nome}{d.uf && <small> · {d.uf}</small>}</div>
{d.vento != null && <div className="dado"><span className="seta" style={{ rotate: `${d.dir}deg` }}>{"\u{F005D}"}</span>{Math.round(d.vento)} km/h</div>}
{d.media != null && <div className="dado">média do dia {grau(d.media)}</div>}
</div>
</section>
{d.horas && <section id="horas">
<div className="serie mm"><BarChart data={serie(x => x.mm)} label="Chuva" period={period} unit="mm" averageLabel="Chuva, média por hora" valueLabel="Chuva" categoryLabel="Hora" showAverage={false} height={84} formatValue={v => n1.format(v)} /></div>
<div className="serie chance"><BarChart data={serie(x => Math.round(x.chance * 100))} label="Chance de chuva" period={period} unit="%" averageLabel="Chance de chuva, média" valueLabel="Chance de chuva" categoryLabel="Hora" height={84} /></div>
<div className="serie temp-h"><BarChart data={serie(x => x.t)} label="Temperatura" period={period} averageLabel="Temperatura, média do dia" valueLabel="Temperatura" categoryLabel="Hora" height={84} formatValue={grau} /></div>
</section>}
</>;
}
