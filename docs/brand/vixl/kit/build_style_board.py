from vx import *; from art import *
PAPER="#f2e5c6"; RED="#b5332a"; GREEN="#4f6b3a"; MUST="#e3a83a"; INK="#3a2a1e"; SHADOW="#6e1d17"; MUTED="#6d5a40"
F=[("Alfa Slab One",400),("Lobster",400),("Oswald",700),("Courier Prime",400)]
o=[{"type":"solid","name":"paper","color":PAPER},{"type":"look","target":"paper","look":"paper"},
   rect("frame-out",20,20,1560,960,"transparent",stroke=RED,stroke_width=8),
   rect("frame-in",36,36,1528,928,"transparent",stroke=RED,stroke_width=2)]
# header
o+=[{"type":"shape","shape":"ribbon","name":"rib","x":70,"y":70,"width":520,"height":110,"fill":RED},
    {"type":"text","name":"word","text":"TRACTR","size":68,"color":PAPER,"font":"alfa-slab-one-400","tracking":4,"within":"rib"},
    {"type":"layer-style","target":"word","name":"drop-shadow","settings":{"dx":3,"dy":3,"blur":0,"opacity":1,"color":SHADOW}},
    text("kicker","STYLE BOARD  ·  DIRECTION B",640,84,22,RED,"oswald-700",tracking=5),
    text("title","Seed Catalog",640,112,60,INK,"lobster-400"),
    text("desc","1950s seed packets and farm-supply ads: warm paper, flat ink colours, sunbursts, ribbons and a red tractor.",1110,84,17,MUTED,"courier-prime-400",line_height=1.45),
    {"type":"text-layout","target":"desc","width":420}]
# palette
sw=[("PAPER","#f2e5c6",INK),("BARN RED","#b5332a",PAPER),("FIELD GREEN","#4f6b3a",PAPER),("MUSTARD","#e3a83a",INK),
    ("SPROUT","#7d9440",INK),("WHEAT","#c58f2c",INK),("SHADOW RED","#6e1d17",PAPER),("INK","#3a2a1e",PAPER)]
o.append(text("pal-l","PALETTE",70,215,16,MUTED,"oswald-700",tracking=5))
for i,(n,c,tc) in enumerate(sw):
    x=70+i*182
    o.append(rect(f"sw{i}",x,240,170,120,c,r=8,**({"stroke":"#c9b48a","stroke_width":2} if c==PAPER else {})))
    o.append(text(f"sw{i}-n",n,x+14,306,15,tc,"oswald-700",tracking=2))
    o.append(text(f"sw{i}-h",c.upper(),x+14,330,15,tc,"courier-prime-400"))
# type specimens
o.append(text("type-l","TYPE",70,395,16,MUTED,"oswald-700",tracking=5))
spec=[("alfa-slab-one-400","HEADLINES",46,{"tracking":3},"Alfa Slab One · display, caps"),
      ("lobster-400","Grow your own worlds!",44,{},"Lobster · short exclamations only"),
      ("oswald-700","LABELS · KICKERS · BUTTONS",30,{"tracking":7},"Oswald 700 · caps, tracked"),
      ("courier-prime-400","Body copy, data, values and code.",26,{},"Courier Prime · body and numbers")]
y=425
for i,(fnt,s,sz,kw,cap) in enumerate(spec):
    o.append(text(f"spec{i}",s,70,y,sz,INK if i!=0 else RED,fnt,**kw))
    o.append(text(f"spec{i}-c",cap,70,y+sz+10,14,MUTED,"courier-prime-400"))
    y+=sz+52
o.append({"type":"layer-style","target":"spec0","name":"drop-shadow","settings":{"dx":2,"dy":2,"blur":0,"opacity":1,"color":SHADOW}})
# motifs panel
mx,my=820,395
o.append(text("mot-l","MOTIFS",mx,my,16,MUTED,"oswald-700",tracking=5))
o.append(rect("mot-panel",mx,my+30,710,500,"#f7eed6",r=10,stroke="#c9b48a",stroke_width=2))
o+=[{"type":"shape","shape":"sunburst","name":"m-rays","x":mx+10,"y":my+40,"width":230,"height":230,"fill":MUST,"count":20,"opacity":0.6},
    {"type":"look","target":"m-rays","look":"halftone"},
    ell("m-sun",mx+75,my+105,100,100,"#f0c35e"),
    {"type":"shape","shape":"ribbon","name":"m-rib","x":mx+260,"y":my+110,"width":220,"height":70,"fill":RED},
    {"type":"shape","shape":"seal","name":"m-seal","x":mx+520,"y":my+60,"width":170,"height":170,"fill":GREEN,"count":22,"inner_radius":0.88,"rotation":12},
    ell("m-seal-in",mx+540,my+80,130,130,"transparent",stroke=PAPER,stroke_width=3)]
o+=[text("m1","SUNBURST + HALFTONE",mx+25,my+275,12,MUTED,"oswald-700",tracking=2),
    text("m2","RIBBON BANNER",mx+315,my+275,12,MUTED,"oswald-700",tracking=2),
    text("m3","SEAL BADGE (TILTED)",mx+540,my+275,12,MUTED,"oswald-700",tracking=2)]
C={"body":RED,"dark":INK,"tire":"#2a1f18","rim":MUST,"hub":"#8f2a22","light":PAPER,"lamp":"#fff4d6"}
o+=[{"type":"shape","shape":"half-circle","name":"m-hill","x":mx+255,"y":my+452,"width":420,"height":78,"fill":GREEN}]
for i in range(4):
    o.append({"type":"shape","shape":"path","name":f"m-row{i}","x":0,"y":0,"path":f"M {mx+320+i*80} {my+522} Q {mx+370+i*60} {my+490} {mx+410+i*45} {my+462}","stroke":"#3d5530","stroke_width":6,"line_cap":"round"})
o+=tractor("m-tr",mx+340,my+290,0.42,C)
for i,(xx,l) in enumerate([(mx+60,0.05),(mx+90,-0.08),(mx+120,0.1)]):
    o+=wheat(f"m-wh{i}",xx,my+480,170,"#c58f2c",lean=l,grains=7,stem="#a8772a",sw=3)
o+=[text("m4","WHEAT, FLANKING",mx+40,my+496,12,MUTED,"oswald-700",tracking=2)]
o.append({"type":"reorder","target":"m4","above":"m-wh2-grain"})
o+=[text("m5","RED TRACTOR · ROLLING ROWS",mx+395,my+500,12,PAPER,"oswald-700",tracking=2)]
# rules strip
o.append(text("rules","FLAT FILLS  ·  HARD OFFSET SHADOWS, NO BLUR\nPAPER GRAIN  ·  CENTRED, FRAMED, POSTER-LIKE",70,840,18,RED,"oswald-700",tracking=4,line_height=1.6))
make("tractr-style-board",1600,1000,PAPER,F,o)
