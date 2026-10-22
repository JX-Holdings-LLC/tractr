from vx import *; from art import *; import ui
PAPER="#f2e5c6"; RED="#b5332a"; GREEN="#4f6b3a"; MUST="#e3a83a"; INK="#3a2a1e"
def logo(x,y):
    return [{"type":"shape","shape":"ribbon","name":"logo-rib","x":x,"y":y-2,"width":210,"height":44,"fill":MUST},
            {"type":"text","name":"logo-t","text":"TRACTR","size":28,"color":INK,"font":"alfa-slab-one-400","tracking":3,"within":"logo-rib"}]
t=dict(bg="#e9dab6",bg_texture="paper",bar=RED,bar_text=PAPER,panel="#f7eed6",panel2="#eadcb8",line="#c9b48a",border=2,radius=10,
 rivets=False,head_text=INK,text=INK,muted="#8a7556",accent=RED,on_accent=PAPER,accent2=GREEN,on_accent2=PAPER,
 select="#f0d28c",select_line=RED,on_select=PAPER,well="#e6d6ae",knob=PAPER,code_bg="#fbf5e4",code_text=GREEN,
 gauge_face="#fbf5e4",needle=RED,
 head="lobster-400",body="courier-prime-400",body_b="oswald-700",mono="courier-prime-400",upper=False,logo=logo,menu_char=10.5,
 grass="#9fb069",map_texture="paper",road="#d9bf8a",soil="#9b6b42",soil_dark="#865a35",crop="#4f6b3a",crop2="#6d8a3e",grain=MUST,
 barn=RED,barn_dark="#7e211b",metal="#b8b2a0",metal_dark="#8f8a7a",house="#f7eed6",water="#7fa6a6",water_edge="#5d8787",tree="#5e7a3a",tree_dark="#46602b",
 tire=INK,tractor=RED,grid=INK,grid_opacity=0.08,
 spring="#8aa456",summer=MUST,fall="#d9772f",winter="#9db3b5",season_text=INK)
ops=ui.build(t)
# menu text on red bar; transport buttons need contrast on red
for op in ops:
    if op.get("name") in ("btn-play",): op["fill"]=MUST
    if op.get("name") in ("btn-play-ico",): op["fill"]=INK
    if op.get("name") in ("daylabel",): op["color"]=PAPER
make("tractr-editor-concept",1600,1000,PAPER,[("Alfa Slab One",400),("Lobster",400),("Oswald",700),("Courier Prime",400)],ops)
