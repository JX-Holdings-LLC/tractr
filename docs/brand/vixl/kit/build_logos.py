from vx import *; from art import *
PAPER="#f2e5c6"; RED="#b5332a"; GREEN="#4f6b3a"; MUST="#e3a83a"; INK="#3a2a1e"; SHADOW="#6e1d17"
F=[("Alfa Slab One",400),("Lobster",400),("Oswald",700),("Courier Prime",400)]
T="transparent"
def ribbon(rib, txt, shadow, tagline=None, tag_col=INK, h=400):
    o=[{"type":"shape","shape":"ribbon","name":"ribbon","x":40,"y":40,"width":1120,"height":220,"fill":rib},
       {"type":"text","name":"wordmark","text":"TRACTR","size":146,"color":txt,"font":"alfa-slab-one-400","tracking":8,"within":"ribbon"}]
    if shadow:
        o.append({"type":"layer-style","target":"wordmark","name":"drop-shadow","settings":{"dx":6,"dy":6,"blur":0,"opacity":1,"color":shadow}})
    if tagline:
        o+=[text("tagline","Grow your own worlds!",0,280,80,tag_col,"lobster-400"),center_x("tagline")]
    return o
variants=[("tractr-logo-primary",RED,PAPER,SHADOW,None,400),
          ("tractr-logo-primary-tagline",RED,PAPER,SHADOW,INK,420),
          ("tractr-logo-reverse",PAPER,RED,None,None,400),
          ("tractr-logo-reverse-tagline",PAPER,RED,None,PAPER,420),
          ("tractr-logo-ink",INK,PAPER,None,None,400)]
for name,rib,txt,sh,tag,h in variants:
    make(name,1200,300 if not tag else h,T,F,ribbon(rib,txt,sh,tagline=bool(tag),tag_col=tag or INK))
# seal icon, transparent
seal=[{"type":"shape","shape":"seal","name":"seal","x":24,"y":24,"width":976,"height":976,"fill":GREEN,"count":24,"inner_radius":0.88},
      ell("seal-ring",120,120,784,784,T,stroke=PAPER,stroke_width=14),
      ell("seal-core",150,150,724,724,MUST),
      {"type":"text","name":"seal-t","text":"T","size":560,"color":RED,"font":"alfa-slab-one-400","within":"seal-core"},
      {"type":"layer-style","target":"seal-t","name":"drop-shadow","settings":{"dx":14,"dy":14,"blur":0,"opacity":1,"color":SHADOW}}]
make("tractr-icon-seal",1024,1024,T,F,seal)
# app icon on paper tile
app=[{"type":"shape","shape":"rounded-rectangle","name":"tile","x":0,"y":0,"width":1024,"height":1024,"radius":220,"fill":PAPER},
     {"type":"look","target":"tile","look":"paper"}]
app+=[{"type":"shape","shape":"seal","name":"seal","x":112,"y":112,"width":800,"height":800,"fill":GREEN,"count":24,"inner_radius":0.88},
      ell("seal-ring",190,190,644,644,T,stroke=PAPER,stroke_width=12),
      ell("seal-core",215,215,594,594,MUST),
      {"type":"text","name":"seal-t","text":"T","size":460,"color":RED,"font":"alfa-slab-one-400","within":"seal-core"},
      {"type":"layer-style","target":"seal-t","name":"drop-shadow","settings":{"dx":12,"dy":12,"blur":0,"opacity":1,"color":SHADOW}}]
make("tractr-app-icon",1024,1024,T,F,app)
# illustration kit: tractor and wheat, transparent
C={"body":RED,"dark":INK,"tire":"#2a1f18","rim":MUST,"hub":"#8f2a22","light":PAPER,"lamp":"#fff4d6"}
make("tractr-art-tractor",1240,880,T,F,tractor("tr",20,20,2.0,C))
make("tractr-art-wheat",300,760,T,F,wheat("wh",150,740,700,"#c58f2c",lean=0.06,grains=8,stem="#a8772a",sw=8))
