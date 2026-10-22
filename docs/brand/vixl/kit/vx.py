"""Minimal Vixl CLI driver: every build script calls make() once per document."""
import json, subprocess, os, sys
D = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(D, "out")

def run(*args):
    r = subprocess.run(["vixl", "--json", *args], capture_output=True, text=True, cwd=D)
    if r.returncode != 0:
        print("FAILED", args[:4], r.stdout[-3000:], r.stderr[-3000:]); sys.exit(1)
    return r.stdout

def make(name, w, h, bg, fonts, ops):
    """Create out/NAME.vixl, install the fonts, apply ops atomically, export out/NAME.png."""
    f = os.path.join(OUT, name + ".vixl")
    for p in (f, os.path.join(OUT, name + ".png")):
        if os.path.exists(p): os.remove(p)
    run("new", f"{w}x{h}", "--background", bg, "-o", f)
    for fam, wt in fonts:
        run("-p", f, "font", "install", fam, "--weight", str(wt))
    opf = os.path.join(OUT, name + ".ops.json")
    json.dump(ops, open(opf, "w"), indent=1)
    out = run("-p", f, "apply", opf)
    res = json.loads(out)
    if res.get("warnings"): print("warnings:", json.dumps(res["warnings"])[:1500])
    run("-p", f, "export", os.path.join(OUT, name + ".png"))
    return f
