from vx import *; from art import *
PAPER="#f2e5c6"; RED="#b5332a"; GREEN="#4f6b3a"; MUST="#e3a83a"; INK="#3a2a1e"
F=[("Alfa Slab One",400),("Lobster",400),("Oswald",700),("Courier Prime",400)]
ops=[
 {"type":"solid","name":"paper","color":PAPER},
 {"type":"shape","shape":"sunburst","name":"rays","x":-260,"y":-160,"width":1600,"height":1600,"fill":MUST,"count":28,"opacity":0.55},
 {"type":"look","target":"rays","look":"halftone"},
 ell("sun",290,330,500,500,"#f0c35e"),
 {"type":"shape","shape":"ellipse","name":"hill2","x":-300,"y":760,"width":1100,"height":360,"fill":"#7d9440"},
 {"type":"shape","shape":"ellipse","name":"hill","x":280,"y":790,"width":1200,"height":380,"fill":GREEN},
]
# crop rows on hill
for i in range(7):
    ops.append({"type":"shape","shape":"path","name":f"row{i}","x":0,"y":0,"path":f"M {300+i*120} 1160 Q {520+i*90} 900 {700+i*70} 815","stroke":"#3d5530","stroke_width":10,"line_cap":"round"})
ops+=[rect("band",0,1010,1080,340,PAPER)]
C={"body":RED,"dark":INK,"tire":"#2a1f18","rim":MUST,"hub":"#8f2a22","light":PAPER,"lamp":"#fff4d6"}
ops+=tractor("tr",150,415,1.3,C)
for i,(xx,l) in enumerate([(70,0.05),(105,-0.08),(140,0.1),(940,-0.05),(975,0.08),(1010,-0.1)]):
    ops+=wheat(f"wh{i}",xx,990,300,"#c58f2c",lean=l,grains=8,stem="#a8772a",sw=4)
ops+=[
 rect("frame-out",24,24,1032,1302,"transparent",stroke=RED,stroke_width=10),
 rect("frame-in",46,46,988,1258,"transparent",stroke=RED,stroke_width=2),
 text("est","EST. 2026  ·  SEED No. 01",0,78,24,RED,"oswald-700",tracking=8), center_x("est"),
 text("script","Grow your own worlds!",0,112,96,INK,"lobster-400"), center_x("script"),
 {"type":"shape","shape":"ribbon","name":"ribbon","x":60,"y":930,"width":960,"height":190,"fill":RED},
 {"type":"text","name":"wordmark","text":"TRACTR","size":150,"color":PAPER,"font":"alfa-slab-one-400","tracking":10,"within":"ribbon"},
 {"type":"layer-style","target":"wordmark","name":"drop-shadow","settings":{"dx":5,"dy":5,"blur":0,"opacity":1,"color":"#6e1d17"}},
 text("sub","THE FARM GAME ENGINE",0,1150,50,INK,"oswald-700",tracking=12), center_x("sub"),
 text("sub2","Plant crops · Raise critters · Ship to every platform",0,1222,26,INK,"courier-prime-400"), center_x("sub2"),
 {"type":"shape","shape":"seal","name":"seal","x":800,"y":250,"width":210,"height":210,"fill":GREEN,"count":24,"inner_radius":0.88,"rotation":12},
 ell("seal-in",822,272,166,166,"transparent",stroke=PAPER,stroke_width=3,rotation=12),
 {"type":"text","name":"seal-t","text":"FREE &\nOPEN\nSOURCE","size":30,"color":PAPER,"font":"oswald-700","align":"center","line_height":1.0,"within":"seal","rotation":12,"tracking":2},
 {"type":"look","target":"paper","look":"paper"},
]
# scenery bleeds off the edge on purpose and sits behind the type
ops+=[{"type":"layer-intent","targets":["hill","hill2","row6","rays"],"allow_crop":True},
      {"type":"layer-intent","targets":["paper","rays","sun","hill","hill2","band","frame-out","frame-in"]+[f"row{i}" for i in range(7)],"role":"background"}]
make("tractr-poster",1080,1350,PAPER,F,ops)
