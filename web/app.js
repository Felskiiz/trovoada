import * as maplibregl from './vendor/maplibre-gl.mjs';
import {G,rgb,mix,sst,amostra,rampa,pinta,cantos,frentes,vento,suave,centros} from './camadas.js';
// 6 passos de 30 min por quadro de 3 h; o radar anda quadro a quadro
const K=()=>S.modo?1:6,k1=()=>S.modo?1e3:300;
// entre dois quadros: campos interpolados linearmente; frentes do mais próximo
const mistura=(a,b,r)=>{const L=k=>a[k]&&b[k]?a[k].map((v,j)=>v==null?b[k][j]:b[k][j]==null?v:v+(b[k][j]-v)*r):a[k]||b[k],o={...(r<.5?a:b)};for(const k of['chuva','u','v','am','risco','votos_chuva','fontes'])o[k]=L(k);return o};
const $=s=>document.querySelector(s);
const P={},C={};
const pega=async u=>{try{const r=await fetch(u);return r.ok?await r.json():null}catch{return null}};
const DIAS=['dom','seg','ter','qua','qui','sex','sáb'];
const data=t=>new Date(typeof t=='number'?t*1e3:t);
const hora=t=>{const d=data(t),m=d.getMinutes();return DIAS[d.getDay()]+' '+d.getHours()+'h'+(m?String(m).padStart(2,'0'):'')};
const relogio=t=>{const d=data(t);return d.getHours()+'h'+String(d.getMinutes()).padStart(2,'0')};
const sem=s=>s.normalize('NFD').replace(/[̀-ͯ]/g,'').toLowerCase();
const esc=s=>String(s??'').replace(/[&<>"]/g,c=>({'&':'&amp;','<':'&lt;','>':'&gt;','"':'&quot;'}[c]));
const num=(v,d=0)=>v.toLocaleString('pt-BR',{maximumFractionDigits:d,minimumFractionDigits:d});
const med=a=>{if(!a.length)return null;a=[...a].sort((x,y)=>x-y);const m=a.length>>1;return a.length%2?a[m]:(a[m-1]+a[m])/2};
// INMET manda riscos/instrucoes como repr de lista Python: "['a.', 'b.']"
const lista=s=>Array.isArray(s)?s:String(s??'').trim().replace(/^\[\s*['"]?|['"]?\s*\]$/g,'').split(/['"],\s*['"]/).map(x=>x.trim()).filter(Boolean);
const CHIPS={chuva:'chuva',radar:'radar',ir:'satélite',vento:'vento',massas:'massas de ar',tempestade:'tempestade',raios:'raios',alertas:'alertas'};
const CAPAS={chuva:['chuva'],radar:['radar'],ir:['ir'],vento:[],massas:['massas','frentes-b','frentes','frentes-s'],tempestade:['tempestade'],raios:['raios'],alertas:['alertas-f','alertas-l']};
const PREV=['chuva','vento','massas','tempestade'];
const VZ={type:'FeatureCollection',features:[]};
const VAZIO=Object.assign(document.createElement('canvas'),{width:1,height:1}).toDataURL();
const S={tempos:[],radar:[],i:0,modo:null,campo:null,agora:null,lugar:null,ponto:null,alertas:null,fontes:null,lig:new Set(['chuva','vento']),ultima:'chuva',munis:[],sug:[],sel:0,cache:new Map(),anim:null};
async function cores(){const c=await pega('/api/cores');if(!c)return false;Object.assign(P,c);for(const k in P){if(!/^#[0-9a-f]{6}$/i.test(P[k]))continue;C[k]=rgb(P[k]);document.documentElement.style.setProperty('--'+k.replace('_','-'),P[k])}return true}
while(!await cores())await new Promise(r=>setTimeout(r,2e3));
const R=(id,o=1)=>({id,type:'raster',source:id,layout:{visibility:'none'},paint:{'raster-opacity':o,'raster-fade-duration':0}});
const im=()=>({type:'image',url:VAZIO,coordinates:cantos()}),gj=()=>({type:'geojson',data:VZ});
const map=new maplibregl.Map({container:'mapa',style:{version:8,sources:{paises:{type:'geojson',data:'dados/paises.json'},ufs:{type:'geojson',data:'dados/ufs.json'},chuva:im(),massas:im(),tempestade:im(),radar:{type:'raster',tiles:['/tile/radar/0/{z}/{x}/{y}.png'],tileSize:256,maxzoom:7},ir:{type:'raster',tiles:['/tile/ir/0/{z}/{x}/{y}.png'],tileSize:256,maxzoom:6},frentes:gj(),alertas:gj(),raios:gj()},
layers:[{id:'fundo',type:'background'},{id:'paises',type:'fill',source:'paises'},{id:'ufs',type:'fill',source:'ufs'},R('ir',.85),R('massas'),R('chuva'),R('tempestade'),R('radar',.85),{id:'ufs-l',type:'line',source:'ufs',paint:{'line-opacity':.14,'line-width':.6}},{id:'paises-l',type:'line',source:'paises',paint:{'line-opacity':.32,'line-width':.8}},{id:'frentes-b',type:'line',source:'frentes',layout:{visibility:'none','line-cap':'round','line-join':'round'},paint:{'line-width':5.5,'line-opacity':.9,'line-blur':.5}},{id:'frentes',type:'line',source:'frentes',layout:{visibility:'none','line-cap':'round','line-join':'round'},paint:{'line-width':2.6}},{id:'frentes-s',type:'symbol',source:'frentes',layout:{visibility:'none','symbol-placement':'line','symbol-spacing':70,'icon-image':['match',['get','tipo'],-1,'fria','quente'],'icon-rotation-alignment':'map','icon-allow-overlap':true,'icon-ignore-placement':true}},{id:'alertas-f',type:'fill',source:'alertas',layout:{visibility:'none'},paint:{'fill-opacity':['match',['get','nivel'],2,.3,1,.2,.14]}},{id:'alertas-l',type:'line',source:'alertas',layout:{visibility:'none'},paint:{'line-width':['match',['get','nivel'],2,1.8,1.2],'line-opacity':.9}},{id:'raios',type:'circle',source:'raios',layout:{visibility:'none'},paint:{'circle-radius':2.4,'circle-opacity':['interpolate',['linear'],['get','idade'],0,1,1800,.12]}}]},
bounds:[[-75,-47],[-28,5.5]],fitBoundsOptions:{padding:{top:60,bottom:70,left:20,right:innerWidth>900?414:20}},maxBounds:[[-110.25,-58.25],[.25,14.25]],attributionControl:false,dragRotate:false,maxPitch:0,pitchWithRotate:false,renderWorldCopies:false});
map.touchZoomRotate.disableRotation();map.on('error',()=>{});
const cv=Object.assign(document.createElement('canvas'),{id:'vento'}),tela=document.createElement('canvas');
const marca=new maplibregl.Marker({element:Object.assign(document.createElement('div'),{className:'lugar'})});
let V;
// símbolo sinótico acima do eixo do ícone: triângulo na fria, semicírculo na quente
function simbolo(nome,cor,tri){const c=document.createElement('canvas');c.width=c.height=56;const x=c.getContext('2d');x.fillStyle=cor;x.strokeStyle=P.breu;x.lineWidth=3;x.beginPath();if(tri){x.moveTo(12,29);x.lineTo(28,8);x.lineTo(44,29)}else x.arc(28,29,14,Math.PI,0);x.closePath();x.stroke();x.fill();const d=x.getImageData(0,0,56,56);map.hasImage(nome)?map.updateImage(nome,d):map.addImage(nome,d,{pixelRatio:2})}
function tinta(){simbolo('fria',P.areia,1);simbolo('quente',P.ferro,0);const nv=['match',['get','nivel'],0,P.ocre,P.ferro];for(const[l,k,v]of[['fundo','background-color',P.breu],['paises','fill-color',P.barro],['ufs','fill-color',P.limo],['ufs-l','line-color',P.areia],['paises-l','line-color',P.areia],['frentes-b','line-color',P.breu],['frentes','line-color',['match',['get','tipo'],-1,P.areia,P.ferro]],['raios','circle-color',P.acento_forte],['alertas-f','fill-color',nv],['alertas-l','line-color',nv]])map.setPaintProperty(l,k,v)}
// cor do INMET só decide o nível; a cor final vem da paleta
function nivel(p){const s=sem(`${p.severidade||''} ${p.cor||''}`);return /grande|vermelh|#f00\b|#ff0000/.test(s)?2:/potencial|amarel|#ff0\b|#ffff00/.test(s)?0:1}
// rampa contínua como Windy/RainViewer: verde na garoa, opacidade sobe com a intensidade; média de 3 h quase nunca passa de 10 mm/h
const CH=()=>[[.2,mix(C.limo,C.musgo,.6)],[1,C.musgo],[3,C.acento],[6,C.ocre],[12,C.ferro],[25,C.areia]];
const ROT=[];function rotulos(l){ROT.splice(0).forEach(m=>m.remove());for(const c of l){const e=document.createElement('div');e.className='massa '+(c.v<0?'f':'q');e.textContent=(c.v<0?'massa fria ':'massa quente ')+(c.v<0?'−':'+')+Math.abs(Math.round(c.v))+'°';ROT.push(new maplibregl.Marker({element:e}).setLngLat([c.lon,c.lat]).addTo(map))}}
function desenha(){const f=S.campo,img=(k,ok,fn,b)=>map.getSource(k).updateImage({url:S.lig.has(k)&&f&&ok&&!S.modo?pinta(tela,fn,8,b):VAZIO,coordinates:cantos()});
img('chuva',f?.chuva,f?.chuva&&(()=>{const st=CH(),ch=suave(f.chuva);return(x,y)=>{const v=amostra(ch,x,y);if(v==null||v<.1)return null;const vo=f.votos_chuva?amostra(f.votos_chuva,x,y):null,fo=f.fontes?amostra(f.fontes,x,y):null,ac=vo!=null&&fo?Math.min(1,vo/fo):1,al=(.5+.45*sst(0,1,Math.log(v/.2)/Math.log(30)))*sst(.1,.3,v)*(.6+.4*ac);return[...rampa(st,v),al]}})(),4);
img('massas',f?.am,(x,y)=>{const a=amostra(f.am,x,y);if(a==null)return null;const t=Math.min(1,Math.abs(a)/7),al=.72*sst(.12,.8,t);return al>.01?[...(a<0?C.areia:mix(C.ocre,C.ferro,sst(.2,1,t))),al]:null});
img('tempestade',f?.risco,(x,y)=>{const r=amostra(f.risco,x,y);if(r==null||r<.5)return null;return[...mix(C.ocre,C.ferro,Math.min(1,Math.max(0,(r-1)/2))),(.18+.14*r)*sst(.5,1,r)]});
map.getSource('frentes').setData(S.lig.has('massas')&&f?.fl&&!S.modo?f.fl:VZ);rotulos(S.lig.has('massas')&&f?.am&&!S.modo?centros(f.am):[]);
V.dados(f?.u,f?.v)}
async function campo(t){if(t==null)return null;if(!S.cache.has(t)){if(S.cache.size>16)S.cache.delete(S.cache.keys().next().value);S.cache.set(t,pega('/api/campo?t='+t).then(f=>{if(!f)S.cache.delete(t);else if(f.frente){f.fl=frentes(f.frente,20,f.t850);f.am=f.anom850.map((a,i)=>f.t850[i]-f.t2m[i]>3?null:a)}return f}))}return S.cache.get(t)}
const agoraIdx=()=>{const n=Date.now()/1e3;let b=0;S.tempos.forEach((t,i)=>{if(Math.abs(t-n)<Math.abs(S.tempos[b]-n))b=i});return b};
function radarEm(t){if(t==null||t===S.rt)return;S.rt=t;map.getSource('radar').setTiles([`/tile/radar/${t}/{z}/{x}/{y}.png`])}
async function vai(){const s=$('#slider');S.i=Math.min(+s.max/K(),S.i);if(S.modo){const q=S.radar[S.i];$('#instante').textContent=relogio(q.t);radarEm(q.t);return}radarEm(S.radar.at(-1)?.t);const i=S.i,a=Math.floor(i+1e-6),r=i-a<1e-6?0:i-a,t0=S.tempos[a],t1=S.tempos[Math.min(a+1,S.tempos.length-1)];$('#instante').textContent=hora(t0+(t1-t0)*r);const[f0,f1]=await Promise.all([campo(t0),r?campo(t1):null]);if(i!==S.i||S.modo)return;S.campo=r&&f0&&f1?mistura(f0,f1,r):f0;desenha();campo(S.tempos[a+1]);campo(S.tempos[a+2])}
// radar sozinho (sem camada de previsão) faz a linha do tempo andar pelos quadros dele
function linha(forca){const r=S.lig.has('radar')&&!PREV.some(k=>S.lig.has(k))&&S.radar.length>0,n=r?S.radar.length:S.tempos.length,s=$('#slider');$('#tempo').hidden=!n;if(!n)return;if(forca||r!==S.modo){S.modo=r;S.i=r?n-1:agoraIdx()}s.max=(n-1)*K();s.value=Math.round(S.i*K());marcas();vai()}
// trilho: passado fino, futuro grosso, rótulo de cada dia à meia-noite e o agora no meio
function marcas(){const s=$('#slider'),el=$('#dias'),m=+s.max/K();if(S.modo||!m){el.innerHTML='';s.style.removeProperty('--ag');return}const a=agoraIdx(),p=i=>`left:calc(7px + (100% - 14px)*${i/m})`;s.style.setProperty('--ag',a/m*100+'%');el.innerHTML=S.tempos.map((t,i)=>data(t).getHours()<3&&Math.abs(i-a)>3?`<span style="${p(i)}">${DIAS[data(t).getDay()]}</span>`:'').join('')+`<span class="ag" style="${p(a)}">agora</span>`}
const sw=k=>`<i style="background:var(--${k})"></i>`;
const LEG={chuva:()=>CH().map(([v,c])=>`<i style="background:rgb(${c})"></i>${String(v).replace('.',',')}`).join('')+' mm/h',vento:()=>`${sw('areia')}0${sw('ocre')}20${sw('ferro')}45 km/h`,massas:()=>`${sw('areia')}fria${sw('ferro')}quente <b style="color:var(--areia)">▲</b> frente fria <b style="color:var(--ferro)">●</b> frente quente · °C em 850 hPa contra a média da latitude`,tempestade:()=>`${sw('ocre')}1${sw('ferro')}3`};
function legenda(){const k=[S.ultima,...S.lig].find(k=>S.lig.has(k)&&LEG[k]),el=$('#legenda');el.hidden=!k;if(k)el.innerHTML=LEG[k]()}
function liga(k,on){on?S.lig.add(k):S.lig.delete(k);if(on)S.ultima=k;$(`#chips [data-k="${k}"]`).setAttribute('aria-pressed',on);for(const l of CAPAS[k])map.setLayoutProperty(l,'visibility',on?'visible':'none');if(k=='vento')V.liga(on);if(k=='raios')raios();legenda();linha()}
async function tempos(){const r=await pega('/api/tempos');if(!r)return;if(r.grade)Object.assign(G,r.grade);S.fontes=r.fontes;rodape();const novo=JSON.stringify(r.tempos)!==JSON.stringify(S.tempos);S.tempos=r.tempos||[];S.cache.clear();S.agora=S.tempos.length?await campo(S.tempos[agoraIdx()]):null;agoraBloco();linha(novo)}
async function passado(){const[r,s]=await Promise.all([pega('/api/radar'),pega('/api/satelite')]);S.radar=Array.isArray(r)?r:[];const ir=s?.ir?.at(-1);if(ir&&ir!==S.irt){S.irt=ir;map.getSource('ir').setTiles([`/tile/ir/${ir}/{z}/{x}/{y}.png`])}linha()}
async function raios(){if(!S.lig.has('raios'))return;const r=await pega('/api/raios');map.getSource('raios').setData({type:'FeatureCollection',features:(r||[]).map(([la,lo,i])=>({type:'Feature',properties:{idade:i},geometry:{type:'Point',coordinates:[lo,la]}}))})}
async function alertas(){const r=await pega('/api/alertas');for(const f of r?.features||[])f.properties.nivel=nivel(f.properties);S.alertas=r;map.getSource('alertas').setData(r?.features?r:VZ);blocoAlertas()}
const anel=(r,x,y)=>{let d=false;for(let i=0,j=r.length-1;i<r.length;j=i++){const[a,b]=r[i],[c,e]=r[j];if((b>y)!==(e>y)&&x<(c-a)*(y-b)/(e-b)+a)d=!d}return d};
const dentro=(g,x,y)=>(g?.type=='Polygon'?[g.coordinates]:g?.type=='MultiPolygon'?g.coordinates:[]).some(p=>anel(p[0],x,y)&&!p.slice(1).some(h=>anel(h,x,y)));
function blocoAlertas(){const el=$('#alertas'),l=S.lugar,fs=l?(S.alertas?.features||[]).filter(f=>dentro(f.geometry,l.lon,l.lat)):[];el.hidden=!fs.length;el.innerHTML=fs.map(f=>{const p=f.properties,rs=lista(p.riscos),is=lista(p.instrucoes);return`<article class="alerta n${p.nivel}"><h3><span class="g">\u{F0026}</span>${esc(p.evento)}</h3><div class="dado">${esc(p.severidade)} · ${p.inicio?hora(p.inicio):''} – ${p.fim?hora(p.fim):''}</div>${rs.map(x=>`<p>${esc(x)}</p>`).join('')}${is.map(x=>`<p>${esc(x)}</p>`).join('')}</article>`}).join('')}
function agoraBloco(){const f=S.agora,l=S.lugar,el=$('#agora');const x=l?Math.round((l.lon-G.lon0)/G.passo):-1,y=l?Math.round((G.lat0-l.lat)/G.passo):-1,i=y*G.nx+x,g=k=>f?.[k]?.[i]??null,t=g('t2m');if(x<0||y<0||x>=G.nx||y>=G.ny||t==null){el.hidden=true;return}
const vo=g('votos_chuva'),fo=g('fontes'),ch=g('chuva'),u=g('u'),v=g('v'),r=g('risco')||0,raj=g('rajada'),cape=g('cape');
el.innerHTML=`<div class="temp">${Math.round(t)}°</div><div class="info"><div class="nome">${esc(l.nome)}${l.uf?`<small> · ${l.uf}</small>`:''}</div>${fo?`<div>${ch>0?`chove em ${vo} de ${fo} fontes <span class="dado">${num(ch,1)} mm/h</span>`:`seco em ${fo-(vo||0)} de ${fo}`}</div>`:''}${u!=null&&v!=null?`<div class="dado"><span class="seta" style="rotate:${Math.atan2(u,v)*180/Math.PI}deg">\u{F005D}</span>${Math.round(Math.hypot(u,v)*3.6)} km/h${raj!=null?` · rajada ${Math.round(raj)}`:''}</div>`:''}${r>0?`<div class="dado r${r}">CAPE ${Math.round(cape??0)} · risco ${r}</div>`:''}</div>`;el.hidden=false}
function horas(){const p=S.ponto,el=$('#horas'),ms=Object.values(p?.modelos||{}).filter(Boolean),n=Math.min(72,p?.horas?.length||0);if(!n||!ms.length){el.hidden=true;return}el.hidden=false;
const ch=[],co=[],te=[];for(let h=0;h<n;h++){const c=ms.map(m=>m.chuva?.[h]).filter(x=>x!=null);ch.push(med(c));co.push(c.length?c.filter(x=>x>=.1).length/c.length:0);te.push(med(ms.map(m=>m.t2m?.[h]).filter(x=>x!=null)))}
const cv=el.querySelector('canvas'),d=devicePixelRatio||1,W=cv.clientWidth,H=130,x=cv.getContext('2d');cv.width=W*d;cv.height=H*d;x.scale(d,d);const bw=W/n,base=H-16,cv2=ch.filter(v=>v!=null),cm=Math.max(2,...cv2);x.font='10px "LXGWWenKaiMono Nerd Font",monospace';x.textBaseline='top';
for(let h=0;h<n;h++){const dt=new Date(p.horas[h]*1e3);if(h==0||dt.getHours()==0){if(h){x.fillStyle=P.limo;x.fillRect(Math.round(h*bw),0,1,base)}x.fillStyle=P.areia;x.fillText(DIAS[dt.getDay()],h*bw+(h?4:0),base+5)}}
x.fillStyle=P.areia;for(let h=0;h<n;h++){const v=ch[h];if(v==null||v<.1)continue;x.globalAlpha=.25+.75*co[h];const bh=Math.max(1.5,v/cm*54);x.fillRect(h*bw+.5,base-bh,Math.max(1,bw-1),bh)}x.globalAlpha=1;
const pk=cv2.length?Math.max(...cv2):0;if(pk>=.2){const h=ch.indexOf(pk),tx=num(pk,1)+' mm/h';x.fillStyle=P.areia;x.textBaseline='bottom';x.fillText(tx,Math.min(W-x.measureText(tx).width,Math.max(0,h*bw-10)),base-pk/cm*54-3)}
const tv=te.filter(v=>v!=null);if(tv.length){const lo=Math.min(...tv),hi=Math.max(...tv),Y=v=>14+(hi-v)/Math.max(1,hi-lo)*(base-84);x.strokeStyle=P.areia;x.lineWidth=1.2;x.lineJoin='round';x.beginPath();te.forEach((v,h)=>{if(v!=null)x[h&&te[h-1]!=null?'lineTo':'moveTo'](h*bw+bw/2,Y(v))});x.stroke();x.fillStyle=P.areia;const rot=(v,b)=>{const h=te.indexOf(v),tx=Math.round(v)+'°';x.textBaseline=b;x.fillText(tx,Math.min(W-x.measureText(tx).width,Math.max(0,h*bw-6)),Y(v)+(b=='bottom'?-3:3))};rot(hi,'bottom');if(hi-lo>=1)rot(lo,'top')}}
function tabela(){const el=$('#fontes'),ok=a=>(a||[]).slice(0,24).filter(x=>x!=null);const rs=Object.entries(S.ponto?.modelos||{}).filter(([,m])=>m).map(([k,m])=>{const c=ok(m.chuva),t=ok(m.t2m),r=ok(m.rajada);if(!c.length&&!t.length)return'';const sc=c.reduce((a,b)=>a+b,0);return`<tr><th>${k.toUpperCase()}</th><td${sc>=1?' class="voto"':''}>${c.length?num(sc,1):'–'}</td><td>${t.length?`${Math.round(Math.min(...t))}° ${Math.round(Math.max(...t))}°`:'–'}</td><td>${r.length?Math.round(Math.max(...r)):'–'}</td></tr>`}).join('');el.hidden=!rs;if(rs)el.querySelector('table').innerHTML=`<thead><tr><td style="text-align:left">24 h</td><td>\u{F058C} mm</td><td>\u{F050F}</td><td>\u{F059D} km/h</td></tr></thead><tbody>${rs}</tbody>`}
const idade=t=>{const h=(Date.now()/1e3-t)/3600;return h<1?Math.max(0,Math.round(h*60))+' min':Math.round(h)+' h'};
function rodape(){const f=S.fontes,el=$('#rodape');el.hidden=!f;if(f)el.innerHTML=Object.entries(f).map(([k,v])=>`<span title="${esc(v?.erro||'')}">${k.toUpperCase()} ${v?.rodada?idade(v.rodada):'–'}${v?.erro?'<i class="erro"></i>':''}</span>`).join('')}
function nomeado(lat,lon,nome){let b=null,d=.04;for(const m of S.munis){const q=(m[2]-lat)**2+((m[3]-lon)*Math.cos(lat*Math.PI/180))**2;if(q<d){d=q;b=m}}return{nome:nome||(b?b[0]:`${num(lat,2)}, ${num(lon,2)}`),uf:b?.[1],lat,lon}}
async function lugar(l,voa){S.lugar=l;marca.setLngLat([l.lon,l.lat]).addTo(map);if(voa)map.flyTo({center:[l.lon,l.lat],zoom:Math.max(map.getZoom(),6),duration:1600,easing:t=>1-(1-t)**4});agoraBloco();blocoAlertas();const p=await pega(`/api/ponto?lat=${l.lat.toFixed(3)}&lon=${l.lon.toFixed(3)}`);if(S.lugar!==l)return;S.ponto=p;horas();tabela()}
const q=$('#q'),ul=$('#sugestoes');
const mostra=()=>{ul.innerHTML=S.sug.map((m,i)=>`<li data-i="${i}"${i==S.sel?' class="sel"':''}>${esc(m[0])}<span> · ${m[1]}</span></li>`).join('');ul.hidden=!S.sug.length};
q.oninput=()=>{const s=sem(q.value.trim());S.sel=0;S.sug=[];if(s){const a=[],b=[];for(const m of S.munis){if(m[4].startsWith(s))a.push(m);else if(b.length<8&&m[4].includes(s))b.push(m);if(a.length==8)break}S.sug=a.concat(b).slice(0,8)}mostra()};
const escolhe=m=>{if(!m)return;q.value='';q.blur();S.sug=[];mostra();lugar({nome:m[0],uf:m[1],lat:m[2],lon:m[3]},true)};
q.onkeydown=e=>{const n=S.sug.length;if(e.key=='ArrowDown'&&n){S.sel=(S.sel+1)%n;mostra();e.preventDefault()}else if(e.key=='ArrowUp'&&n){S.sel=(S.sel-1+n)%n;mostra();e.preventDefault()}else if(e.key=='Enter')escolhe(S.sug[S.sel]);else if(e.key=='Escape'){S.sug=[];mostra()}};
ul.onmousedown=e=>{const li=e.target.closest('li');if(li){e.preventDefault();escolhe(S.sug[li.dataset.i])}};
q.onblur=()=>{S.sug=[];mostra()};
$('#recolhe').onclick=()=>document.body.classList.toggle('sem');
$('#slider').oninput=e=>{S.i=+e.target.value/K();vai()};
$('#play').onclick=()=>{const b=$('#play');if(S.anim){clearInterval(S.anim);S.anim=null}else S.anim=setInterval(()=>{const s=$('#slider'),k=K(),j=(Math.round(S.i*k)+1)%(+s.max+1);S.i=j/k;s.value=j;vai()},k1());b.setAttribute('aria-pressed',!!S.anim);b.textContent=S.anim?'\u{F03E4}':'\u{F040A}'};
$('#chips').innerHTML=Object.entries(CHIPS).map(([k,n])=>`<button data-k="${k}" aria-pressed="${S.lig.has(k)}">${n}</button>`).join('');
$('#chips').onclick=e=>{const k=e.target.dataset?.k;if(k&&V)liga(k,!S.lig.has(k))};
addEventListener('resize',horas);
map.on('click',e=>{const{lng,lat}=e.lngLat;lugar(nomeado(lat,lng),false);if(S.lig.has('alertas')&&map.queryRenderedFeatures(e.point,{layers:['alertas-f']}).length)$('#alertas').scrollIntoView({behavior:'smooth',block:'nearest'})});
map.on('load',async()=>{map.getCanvasContainer().appendChild(cv);V=vento(map,cv,P);tinta();for(const k of S.lig)for(const l of CAPAS[k])map.setLayoutProperty(l,'visibility','visible');V.liga(S.lig.has('vento'));legenda();
const[m,l]=await Promise.all([pega('dados/municipios.json'),pega('/api/lugar'),passado(),alertas()]);S.munis=(m||[]).map(x=>[...x,sem(x[0])]);await tempos();if(l)lugar(nomeado(l.lat,l.lon,l.nome),false);
setInterval(async()=>{if(await cores()){tinta();desenha();horas()}},6e4);setInterval(tempos,6e5);setInterval(passado,3e5);setInterval(raios,6e4);setInterval(alertas,6e5)});
