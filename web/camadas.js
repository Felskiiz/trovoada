export const G={nx:221,ny:145,lat0:14,lon0:-110,passo:.5};
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
// linhas do canvas espaçadas em Mercator: a image source estica linear entre os cantos; o desfoque tira o degrau da grade de 0,5°; desenha no próprio cv, sem PNG
const B=document.createElement('canvas');
export function pinta(cv,fn,e=4,blur=1.5){const p=G.passo,W=G.nx*e,H=G.ny*e,n=merc(G.lat0+p/2),s=merc(G.lat0-(G.ny-.5)*p);B.width=W;B.height=H;const bx=B.getContext('2d'),im=bx.createImageData(W,H),d=im.data;for(let r=0;r<H;r++){const gy=(G.lat0-imerc(n+(s-n)*(r+.5)/H))/p;for(let c=0;c<W;c++){const o=fn((c+.5)/W*G.nx-.5,gy);if(!o)continue;const k=(r*W+c)*4;d[k]=o[0];d[k+1]=o[1];d[k+2]=o[2];d[k+3]=o[3]*255}}bx.putImageData(im,0,0);cv.width=W;cv.height=H;const cx=cv.getContext('2d');cx.filter=blur?`blur(${blur}px)`:'none';cx.drawImage(B,0,0);return cv}
// grade suavizada (3x3 ponderado, ignora null): os degraus das faixas saem curvos em vez de seguir a malha de 0,5°
export function suave(a){const nx=G.nx,ny=G.ny,o=new Array(a.length);for(let y=0;y<ny;y++)for(let x=0;x<nx;x++){let s=0,w=0;for(let j=-1;j<=1;j++)for(let k=-1;k<=1;k++){const yy=y+j,xx=x+k;if(yy<0||xx<0||yy>=ny||xx>=nx)continue;const v=a[yy*nx+xx];if(v==null)continue;const q=(2-Math.abs(j))*(2-Math.abs(k));s+=v*q;w+=q}o[y*nx+x]=w?s/w:null}return o}
// centros das massas: extremos da anomalia suavizada com |a|>=lim num raio de r células
export function centros(a,r=14,lim=3){const nx=G.nx,ny=G.ny,s=suave(suave(a)),o=[];// borda da grade fica de fora: lá o extremo é só o corte
for(let y=8;y<ny-8;y++)for(let x=8;x<nx-8;x++){const v=a[y*nx+x]==null?null:s[y*nx+x];if(v==null||Math.abs(v)<lim)continue;let ok=true;for(let j=-r;j<=r&&ok;j++)for(let k=-r;k<=r;k++){const yy=y+j,xx=x+k;if(yy<0||xx<0||yy>=ny||xx>=nx)continue;const w=s[yy*nx+xx];if(w!=null&&(v>0?w>v:w<v)){ok=false;break}}if(ok)o.push({lon:G.lon0+x*G.passo,lat:G.lat0-y*G.passo,v})}return o.sort((a,b)=>Math.abs(b.v)-Math.abs(a.v)).slice(0,8)}
// frente (-1 fria, 1 quente por célula) em linhas: encadeia vizinhas do mesmo tipo, descarta trecho curto e suaviza (Chaikin)
export function frentes(fr,min=5,t=null,max=8){const nx=G.nx,ny=G.ny,p=G.passo,vis=new Uint8Array(fr.length),f=[];const viz=i=>{const y=i/nx|0,x=i%nx,o=[];for(let j=-1;j<=1;j++)for(let k=-1;k<=1;k++){const yy=y+j,xx=x+k;if((j||k)&&yy>=0&&xx>=0&&yy<ny&&xx<nx&&fr[yy*nx+xx]==fr[i])o.push([yy*nx+xx,j&&k?1:0])}return o};const livre=i=>viz(i).filter(([n])=>!vis[n]);
const pts=a=>a.map(i=>[G.lon0+i%nx*p,G.lat0-(i/nx|0)*p]),chaikin=l=>{const o=[l[0]];for(let i=0;i<l.length-1;i++){const[a,b]=l[i],[c,d]=l[i+1];o.push([.75*a+.25*c,.75*b+.25*d],[.25*a+.75*c,.25*b+.75*d])}o.push(l.at(-1));return o};
for(const ponta of[true,false])for(let i=0;i<fr.length;i++){if(!fr[i]||vis[i]||ponta&&viz(i).length>1)continue;const c=[i];vis[i]=1;for(let q=i;;){const n=livre(q).sort((a,b)=>a[1]-b[1])[0];if(!n)break;q=n[0];vis[q]=1;c.push(q)}if(c.length>=min)f.push({type:'Feature',properties:{tipo:fr[i],n:c.length},geometry:{type:'LineString',coordinates:chaikin(chaikin(chaikin(pts(c.filter((_,i)=>i%3==0||i==c.length-1)))))}})}
f.sort((a,b)=>b.properties.n-a.properties.n).splice(max);if(t)for(const q of f){const l=q.geometry.coordinates,h=l.length>>1,[a,b]=l[Math.max(0,h-3)],[c,d]=l[Math.min(l.length-1,h+3)],n=Math.hypot(c-a,d-b)||1,ox=-(d-b)/n*1.5,oy=(c-a)/n*1.5,[mx,my]=l[h],T=(x,y)=>amostra(t,(x-G.lon0)/p,(G.lat0-y)/p),e=T(mx+ox,my+oy),r=T(mx-ox,my-oy);
// fria avança para o lado quente, quente para o lado frio
if(e!=null&&r!=null&&(q.properties.tipo<0?e<r:e>r))l.reverse()}return{type:'FeatureCollection',features:f}}
export function vento(map,cv,P,N=5000){let U=null,V=null,on=false;const cx=cv.getContext('2d'),dpr=devicePixelRatio||1,ps=Array.from({length:N},()=>({h:[]})),L=22;
const tam=()=>{const c=map.getContainer();cv.width=c.clientWidth*dpr;cv.height=c.clientHeight*dpr;cv.style.width=c.clientWidth+'px';cv.style.height=c.clientHeight+'px'};
const nasce=p=>{p.lon=G.lon0+Math.random()*(G.nx-1)*G.passo;p.lat=G.lat0-Math.random()*(G.ny-1)*G.passo;p.i=Math.random()*200|0;p.h=[]};
const limpa=()=>cx.clearRect(0,0,cv.width,cv.height);
const MX=l=>(l+180)/360,MY=l=>.5-merc(l)/(2*Math.PI);
const passo=()=>{if(on&&U){limpa();const k=.035/2**map.getZoom(),A=map.project([-60,0]),Z=map.project([-30,-30]),ax=MX(-60),ay=MY(0),fx=(Z.x-A.x)/(MX(-30)-ax)*dpr,fy=(Z.y-A.y)/(MY(-30)-ay)*dpr,X=m=>A.x*dpr+(m-ax)*fx,Y=m=>A.y*dpr+(m-ay)*fy;for(const p of ps){const gx=(p.lon-G.lon0)/G.passo,gy=(G.lat0-p.lat)/G.passo,fora=gx<0||gy<0||gx>G.nx-1||gy>G.ny-1,u=fora?null:amostra(U,gx,gy),v=u==null?null:amostra(V,gx,gy);if(v==null||++p.i>260){nasce(p);continue}p.lon+=u*k/Math.cos(p.lat*Math.PI/180);p.lat+=v*k;const mx=MX(p.lon),my=MY(p.lat),s=Math.hypot(u,v)*3.6;p.h.push(mx,my);if(p.h.length>2*L)p.h.splice(0,2);p.c=s<20?0:s<45?1:2}
// rastro explícito das últimas L posições: o fade acumulado (destination-in) deixava resíduo de 8 bits como véu sobre a grade
const cs=[P.areia,P.ocre,P.ferro];cx.lineWidth=1.3*dpr;cx.lineCap='round';for(let a=0;a<L-1;a++){cs.forEach((c,j)=>{cx.globalAlpha=(1-a/(L-1))**1.5*(j?1:.6);cx.strokeStyle=c;cx.beginPath();for(const p of ps){const e=p.h.length/2-1-a;if(p.c!=j||e<1)continue;cx.moveTo(X(p.h[2*e-2]),Y(p.h[2*e-1]));cx.lineTo(X(p.h[2*e]),Y(p.h[2*e+1]))}cx.stroke()})}cx.globalAlpha=1}requestAnimationFrame(passo)};
map.on('resize',tam);tam();requestAnimationFrame(passo);
return{dados(u,v){U=u;V=v},liga(b){on=b;limpa();if(b)ps.forEach(nasce)}}}
