"""Draw the sleeping sprites in the RunCat365 style.

Black silhouettes with soft (anti-aliased) edges on a 32x32 transparent
canvas, drawn at 8x and downsampled. The floating "z" letters are not part
of these images: the applet animates them on top at runtime.

Usage: draw_sleep_sprites.py OUT_DIR
Writes OUT_DIR/cat-sleep.png and OUT_DIR/horse-sleep.png.
"""
import os
import sys
from PIL import Image, ImageDraw

K = 8  # supersampling factor


class Canvas:
    def __init__(self):
        self.img = Image.new("L", (32 * K, 32 * K), 0)
        self.d = ImageDraw.Draw(self.img)

    @staticmethod
    def p(x, y):
        return (x * K, y * K)

    def ell(self, x0, y0, x1, y1, fill=255):
        self.d.ellipse([*self.p(x0, y0), *self.p(x1, y1)], fill=fill)

    def poly(self, *pts, fill=255):
        self.d.polygon([self.p(x, y) for x, y in pts], fill=fill)

    def line(self, pts, w, fill=255):
        self.d.line([self.p(x, y) for x, y in pts], fill=fill,
                    width=int(w * K), joint="curve")
        if fill:
            for x, y in (pts[0], pts[-1]):
                r = w * K / 2
                self.d.ellipse([x * K - r, y * K - r, x * K + r, y * K + r],
                               fill=fill)

    def clear_below(self, y):
        self.d.rectangle([*self.p(0, y), *self.p(32, 32)], fill=0)

    def save(self, path):
        alpha = self.img.resize((32, 32), Image.LANCZOS)
        out = Image.new("RGBA", (32, 32), (0, 0, 0, 0))
        out.putalpha(alpha)
        out.save(path)


def cat():
    c = Canvas()
    ground = 27.0
    # Body: a low, rounded loaf, flat on the ground
    c.ell(2.5, 16.0, 24.0, 33)
    c.ell(2.5, 20.5, 10, 27)                   # rounded haunch at the back
    c.clear_below(ground)
    # Head resting on the front paws, set apart from the body by a thin gap
    c.ell(18.0, 15.4, 28.8, 25.6, fill=0)
    c.ell(19.0, 16.4, 27.8, 24.6)
    c.ell(21.0, 20.6, 29.2, ground)           # chin/paws on the ground
    c.poly((19.6, 19.0), (20.2, 13.0), (23.4, 16.8))   # ears
    c.poly((23.8, 16.6), (27.2, 12.8), (27.6, 19.4))
    # Tail wraps round the front of the body, separated by a thin gap
    tail = [(4.0, 24.4), (6.0, 26.2), (10.0, 26.6), (15.0, 26.4), (19.6, 25.4)]
    c.line(tail, 3.6, fill=0)
    c.line(tail, 1.9)
    return c


def horse():
    c = Canvas()
    ground = 23.5
    # Body lying down, legs folded underneath
    c.ell(5.0, 14.0, 24.0, 27.0)
    c.clear_below(ground)
    # Folded front legs poking out in front of the chest
    c.line([(19.0, 22.4), (25.5, 22.6)], 1.7)
    # Neck rising from the chest, head drooping forward and down
    c.poly((17.0, 17.5), (21.5, 11.5), (25.0, 10.0), (24.5, 14.0), (22.0, 19.0))
    c.line([(23.0, 11.2), (27.0, 14.8), (29.2, 17.6)], 3.3)   # sleepy head
    c.poly((22.6, 10.6), (23.6, 7.6), (25.0, 10.4))           # ear
    # Mane along the top of the neck
    c.line([(17.8, 15.6), (21.4, 11.0), (23.6, 9.8)], 1.1)
    # Tail hanging from the rump
    c.line([(5.6, 16.6), (3.4, 19.0), (2.6, 22.4)], 1.6)
    return c


if __name__ == "__main__":
    out = sys.argv[1]
    cat().save(os.path.join(out, "cat-sleep.png"))
    horse().save(os.path.join(out, "horse-sleep.png"))
