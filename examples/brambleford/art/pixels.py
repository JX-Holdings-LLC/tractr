"""A tiny character-grid canvas for drawing sprites as Vixl `pixel-art` rows.

Each sprite is drawn here with simple primitives (rectangles, ellipses, single pixels) into a grid of
palette symbols; Vixl then renders the rows, adds the outline and exports the PNG.
"""


class Grid:
    def __init__(self, width, height, fill=".", oy=0):
        """`oy` shifts everything drawn down by that many rows (room above row 0 for hats and buns)."""
        self.w = width
        self.h = height
        self.oy = oy
        self.cells = [[fill] * width for _ in range(height)]

    def set(self, x, y, c):
        x, y = int(x), int(y) + self.oy
        if 0 <= x < self.w and 0 <= y < self.h:
            self.cells[y][x] = c

    def get(self, x, y):
        x, y = int(x), int(y) + self.oy
        if 0 <= x < self.w and 0 <= y < self.h:
            return self.cells[y][x]
        return "."

    def rect(self, x0, y0, x1, y1, c, only=None):
        """Fills x0..x1, y0..y1 inclusive; `only` limits it to cells that already hold those symbols."""
        x0, y0, x1, y1 = int(x0), int(y0), int(x1), int(y1)
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                if only is None or self.get(x, y) in only:
                    self.set(x, y, c)

    def ellipse(self, cx, cy, rx, ry, c, only=None, y_min=None, y_max=None):
        for y in range(int(cy - ry) - 1, int(cy + ry) + 2):
            if y_min is not None and y < y_min:
                continue
            if y_max is not None and y > y_max:
                continue
            for x in range(int(cx - rx) - 1, int(cx + rx) + 2):
                dx = (x + 0.5 - cx - 0.5) / rx
                dy = (y + 0.5 - cy - 0.5) / ry
                if dx * dx + dy * dy <= 1.0 and (only is None or self.get(x, y) in only):
                    self.set(x, y, c)

    def pixels(self, points, c):
        for x, y in points:
            self.set(x, y, c)

    def hline(self, x0, x1, y, c):
        self.rect(x0, y, x1, y, c)

    def replace(self, old, new):
        for row in self.cells:
            for i, c in enumerate(row):
                if c == old:
                    row[i] = new

    def mirrored(self):
        g = Grid(self.w, self.h)
        g.cells = [list(reversed(row)) for row in self.cells]
        return g

    def shifted(self, dx, dy):
        g = Grid(self.w, self.h)
        for y in range(self.h):
            for x in range(self.w):
                c = self.cells[y][x]
                if c != ".":
                    g.set(x + dx, y + dy, c)
        return g

    def paste(self, other, ox=0, oy=0):
        for y in range(other.h):
            for x in range(other.w):
                c = other.cells[y][x]
                if c != ".":
                    self.set(x + ox, y + oy, c)

    def scaled(self, k):
        g = Grid(self.w * k, self.h * k)
        for y in range(self.h):
            for x in range(self.w):
                g.rect(x * k, y * k, x * k + k - 1, y * k + k - 1, self.cells[y][x])
        return g

    def rows(self):
        return ["".join(r) for r in self.cells]
