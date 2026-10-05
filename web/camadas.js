export const G={nx:81,ny:81,lat0:6,lon0:-74,passo:.5};
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
// linhas do canvas espaçadas em Mercator: a image source estica linear entre os cantos
export function pinta(cv,fn){const p=G.passo,W=G.nx*4,H=G.ny*4,n=merc(G.lat0+p/2),s=merc(G.lat0-(G.ny-.5)*p);cv.width=W;cv.height=H;const cx=cv.getContext('2d'),im=cx.createImageData(W,H),d=im.data;for(let r=0;r<H;r++){const gy=(G.lat0-imerc(n+(s-n)*(r+.5)/H))/p;for(let c=0;c<W;c++){const o=fn((c+.5)/W*G.nx-.5,gy);if(!o)continue;const k=(r*W+c)*4;d[k]=o[0];d[k+1]=o[1];d[k+2]=o[2];d[k+3]=o[3]*255}}cx.putImageData(im,0,0);return cv.toDataURL()}
// marching squares; segmentos soltos por nível
export function isobaras(a,dp=4){const nx=G.nx,p=G.passo,f=[];let mn=1e9,mx=-1e9;for(const v of a)if(v!=null){if(v<mn)mn=v;if(v>mx)mx=v}const pt=(x,y)=>[G.lon0+x*p,G.lat0-y*p];for(let L=Math.ceil(mn/dp)*dp;L<=mx;L+=dp){const sg=[];for(let y=0;y<G.ny-1;y++)for(let x=0;x<nx-1;x++){const i=y*nx+x,A=a[i],B=a[i+1],C=a[i+nx+1],D=a[i+nx];if(A==null||B==null||C==null||D==null)continue;const q=[],e=(v1,v2,x1,y1,x2,y2)=>{if((v1>=L)!==(v2>=L)){const t=(L-v1)/(v2-v1);q.push(pt(x1+(x2-x1)*t,y1+(y2-y1)*t))}};e(A,B,x,y,x+1,y);e(B,C,x+1,y,x+1,y+1);e(D,C,x,y+1,x+1,y+1);e(A,D,x,y,x,y+1);if(q.length==2)sg.push(q);else if(q.length==4){if(((A+B+C+D)/4>=L)===(A>=L))sg.push([q[0],q[1]],[q[2],q[3]]);else sg.push([q[0],q[3]],[q[1],q[2]])}}if(sg.length)f.push({type:'Feature',properties:{p:L},geometry:{type:'MultiLineString',coordinates:sg}})}return{type:'FeatureCollection',features:f}}
// máximo/mínimo na janela (empate fica com o primeiro do platô), com contraste mínimo para não marcar ruído
export function extremos(a,r=6,dif=3){const nx=G.nx,o=[];for(let y=r;y<G.ny-r;y++)for(let x=r;x<nx-r;x++){const v=a[y*nx+x];if(v==null)continue;let mx=true,mn=true,lo=v,hi=v;for(let j=-r;j<=r;j++)for(let i=-r;i<=r;i++){if(!i&&!j)continue;const w=a[(y+j)*nx+x+i];if(w==null)continue;const ant=j<0||!j&&i<0;if(w>v||w==v&&ant)mx=false;if(w<v||w==v&&ant)mn=false;if(w<lo)lo=w;if(w>hi)hi=w}if(mx&&v-lo>=dif||mn&&hi-v>=dif)o.push({alta:mx,v,lon:G.lon0+x*G.passo,lat:G.lat0-y*G.passo})}return o}
export function vento(map,cv,P,N=4000){let U=null,V=null,on=false,anda=true;const cx=cv.getContext('2d'),dpr=devicePixelRatio||1,ps=Array.from({length:N},()=>({}));
const tam=()=>{const c=map.getContainer();cv.width=c.clientWidth*dpr;cv.height=c.clientHeight*dpr;cv.style.width=c.clientWidth+'px';cv.style.height=c.clientHeight+'px'};
const nasce=p=>{const b=map.getBounds(),w=Math.max(b.getWest(),G.lon0),e=Math.min(b.getEast(),G.lon0+(G.nx-1)*G.passo),s=Math.max(b.getSouth(),G.lat0-(G.ny-1)*G.passo),n=Math.min(b.getNorth(),G.lat0);p.lon=w+Math.random()*(e-w);p.lat=s+Math.random()*(n-s);p.i=Math.random()*90|0;const q=map.project([p.lon,p.lat]);p.x=q.x;p.y=q.y};
const limpa=()=>cx.clearRect(0,0,cv.width,cv.height);
const passo=()=>{if(on&&anda&&U){cx.globalCompositeOperation='destination-in';cx.fillStyle='rgba(0,0,0,.86)';cx.fillRect(0,0,cv.width,cv.height);cx.globalCompositeOperation='source-over';const k=.1/2**map.getZoom(),bs=[[],[],[]];for(const p of ps){const gx=(p.lon-G.lon0)/G.passo,gy=(G.lat0-p.lat)/G.passo,fora=gx<0||gy<0||gx>G.nx-1||gy>G.ny-1,u=fora?null:amostra(U,gx,gy),v=u==null?null:amostra(V,gx,gy);if(v==null||++p.i>110){nasce(p);continue}p.lon+=u*k/Math.cos(p.lat*Math.PI/180);p.lat+=v*k;const q=map.project([p.lon,p.lat]),s=Math.hypot(u,v)*3.6;bs[s<20?0:s<45?1:2].push(p.x,p.y,q.x,q.y);p.x=q.x;p.y=q.y}
const cs=[P.areia,P.ocre,P.ferro];cx.lineWidth=1.2*dpr;cx.lineCap='round';bs.forEach((b,j)=>{cx.strokeStyle=cs[j];cx.beginPath();for(let i=0;i<b.length;i+=4){cx.moveTo(b[i]*dpr,b[i+1]*dpr);cx.lineTo(b[i+2]*dpr,b[i+3]*dpr)}cx.stroke()})}requestAnimationFrame(passo)};
map.on('movestart',()=>{anda=false;limpa()});map.on('moveend',()=>{anda=true;if(on)ps.forEach(nasce)});map.on('resize',tam);tam();requestAnimationFrame(passo);
return{dados(u,v){U=u;V=v},liga(b){on=b;limpa();if(b)ps.forEach(nasce)}}}
