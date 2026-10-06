export const G={nx:113,ny:113,lat0:6,lon0:-76,passo:.5};
export const rgb=h=>[1,3,5].map(i=>parseInt(h.slice(i,i+2),16));
export const mix=(a,b,f)=>a.map((x,i)=>x+(b[i]-x)*f);
export const sst=(a,b,x)=>{const t=Math.min(1,Math.max(0,(x-a)/(b-a)));return t*t*(3-2*t)};
// bilinear ignorando vizinhos null; gx/gy em índices fracionários da grade
export function amostra(a,gx,gy){const n=G.nx-1,m=G.ny-1;gx=gx<0?0:gx>n?n:gx;gy=gy<0?0:gy>m?m:gy;const x0=Math.min(n-1,gx|0),y0=Math.min(m-1,gy|0),fx=gx-x0,fy=gy-y0,i=y0*G.nx+x0;let s=0,w=0,v,q;
v=a[i];q=(1-fx)*(1-fy);if(v!=null){s+=v*q;w+=q}
v=a[i+1];q=fx*(1-fy);if(v!=null){s+=v*q;w+=q}
v=a[i+G.nx];q=(1-fx)*fy;if(v!=null){s+=v*q;w+=q}
v=a[i+G.nx+1];q=fx*fy;if(v!=null){s+=v*q;w+=q}
return w>1e-6?s/w:null}
// stops [[valor,rgb]…] interpolados em log
export function rampa(st,v){if(v<=st[0][0])return[...st[0][1]];for(let i=1;i<st.length;i++)if(v<=st[i][0]){const[a,ca]=st[i-1],[b,cb]=st[i];return mix(ca,cb,Math.log(v/a)/Math.log(b/a))}return[...st.at(-1)[1]]}
const merc=l=>Math.log(Math.tan(Math.PI/4+l*Math.PI/360));
const imerc=y=>Math.atan(Math.sinh(y))*180/Math.PI;
export function cantos(){const p=G.passo,w=G.lon0-p/2,e=G.lon0+(G.nx-.5)*p,n=G.lat0+p/2,s=G.lat0-(G.ny-.5)*p;return[[w,n],[e,n],[e,s],[w,s]]}
// linhas do canvas espaçadas em Mercator: a image source estica linear entre os cantos; o desfoque tira o degrau da grade de 0,5°
const B=document.createElement('canvas');
export function pinta(cv,fn,e=8,blur=3){const p=G.passo,W=G.nx*e,H=G.ny*e,n=merc(G.lat0+p/2),s=merc(G.lat0-(G.ny-.5)*p);cv.width=W;cv.height=H;const cx=cv.getContext('2d'),im=cx.createImageData(W,H),d=im.data;for(let r=0;r<H;r++){const gy=(G.lat0-imerc(n+(s-n)*(r+.5)/H))/p;for(let c=0;c<W;c++){const o=fn((c+.5)/W*G.nx-.5,gy);if(!o)continue;const k=(r*W+c)*4;d[k]=o[0];d[k+1]=o[1];d[k+2]=o[2];d[k+3]=o[3]*255}}cx.putImageData(im,0,0);if(!blur)return cv.toDataURL();B.width=W;B.height=H;const b=B.getContext('2d');b.filter=`blur(${blur}px)`;b.drawImage(cv,0,0);return B.toDataURL()}
// frente (-1 fria, 1 quente por célula) em linhas: encadeia vizinhas do mesmo tipo, descarta trecho curto e suaviza (Chaikin)
export function frentes(fr,min=5){const nx=G.nx,ny=G.ny,p=G.passo,vis=new Uint8Array(fr.length),f=[];const viz=i=>{const y=i/nx|0,x=i%nx,o=[];for(let j=-1;j<=1;j++)for(let k=-1;k<=1;k++){const yy=y+j,xx=x+k;if((j||k)&&yy>=0&&xx>=0&&yy<ny&&xx<nx&&fr[yy*nx+xx]==fr[i])o.push([yy*nx+xx,j&&k?1:0])}return o};const livre=i=>viz(i).filter(([n])=>!vis[n]);
const pts=a=>a.map(i=>[G.lon0+i%nx*p,G.lat0-(i/nx|0)*p]),chaikin=l=>{const o=[l[0]];for(let i=0;i<l.length-1;i++){const[a,b]=l[i],[c,d]=l[i+1];o.push([.75*a+.25*c,.75*b+.25*d],[.25*a+.75*c,.25*b+.75*d])}o.push(l.at(-1));return o};
for(const ponta of[true,false])for(let i=0;i<fr.length;i++){if(!fr[i]||vis[i]||ponta&&viz(i).length>1)continue;const c=[i];vis[i]=1;for(let q=i;;){const n=livre(q).sort((a,b)=>a[1]-b[1])[0];if(!n)break;q=n[0];vis[q]=1;c.push(q)}if(c.length>=min)f.push({type:'Feature',properties:{tipo:fr[i]},geometry:{type:'LineString',coordinates:chaikin(chaikin(chaikin(pts(c.filter((_,i)=>i%3==0||i==c.length-1)))))}})}return{type:'FeatureCollection',features:f}}
export function vento(map,cv,P,N=2500){let U=null,V=null,on=false,anda=true;const cx=cv.getContext('2d'),dpr=devicePixelRatio||1,ps=Array.from({length:N},()=>({h:[]})),L=12;
const tam=()=>{const c=map.getContainer();cv.width=c.clientWidth*dpr;cv.height=c.clientHeight*dpr;cv.style.width=c.clientWidth+'px';cv.style.height=c.clientHeight+'px'};
const nasce=p=>{const b=map.getBounds(),w=Math.max(b.getWest(),G.lon0),e=Math.min(b.getEast(),G.lon0+(G.nx-1)*G.passo),s=Math.max(b.getSouth(),G.lat0-(G.ny-1)*G.passo),n=Math.min(b.getNorth(),G.lat0);p.lon=w+Math.random()*(e-w);p.lat=s+Math.random()*(n-s);p.i=Math.random()*200|0;p.h=[]};
const limpa=()=>cx.clearRect(0,0,cv.width,cv.height);
const passo=()=>{if(on&&anda&&U){limpa();const k=.035/2**map.getZoom();for(const p of ps){const gx=(p.lon-G.lon0)/G.passo,gy=(G.lat0-p.lat)/G.passo,fora=gx<0||gy<0||gx>G.nx-1||gy>G.ny-1,u=fora?null:amostra(U,gx,gy),v=u==null?null:amostra(V,gx,gy);if(v==null||++p.i>260){nasce(p);continue}p.lon+=u*k/Math.cos(p.lat*Math.PI/180);p.lat+=v*k;const q=map.project([p.lon,p.lat]),s=Math.hypot(u,v)*3.6;p.h.push(q.x*dpr,q.y*dpr);if(p.h.length>2*L)p.h.splice(0,2);p.c=s<20?0:s<45?1:2}
// rastro explícito das últimas L posições: o fade acumulado (destination-in) deixava resíduo de 8 bits como véu sobre a grade
const cs=[P.areia,P.ocre,P.ferro];cx.lineWidth=2.2*dpr;cx.lineCap='round';for(let a=0;a<L-1;a++){cs.forEach((c,j)=>{cx.globalAlpha=(1-a/(L-1))*(j?1:.5);cx.strokeStyle=c;cx.beginPath();for(const p of ps){const e=p.h.length/2-1-a;if(p.c!=j||e<1)continue;cx.moveTo(p.h[2*e-2],p.h[2*e-1]);cx.lineTo(p.h[2*e],p.h[2*e+1])}cx.stroke()})}cx.globalAlpha=1}requestAnimationFrame(passo)};
map.on('movestart',()=>{anda=false;limpa()});map.on('moveend',()=>{anda=true;if(on)ps.forEach(nasce)});map.on('resize',tam);tam();requestAnimationFrame(passo);
return{dados(u,v){U=u;V=v},liga(b){on=b;limpa();if(b)ps.forEach(nasce)}}}
