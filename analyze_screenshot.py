from PIL import Image
from collections import Counter

img = Image.open('screenshots/gba/20260303141756621.png')
print(f'Size: {img.size}')
pixels = list(img.getdata())
non_white = sum(1 for p in pixels if p[:3] != (255, 255, 255))
non_black = sum(1 for p in pixels if p[:3] != (0, 0, 0))
print(f'Total pixels: {len(pixels)}')
print(f'Non-white pixels: {non_white}')
print(f'Non-black pixels: {non_black}')
color_counts = Counter(p[:3] for p in pixels)
print(f'Unique colors: {len(color_counts)}')
for color, count in color_counts.most_common(15):
    print(f'  RGB({color[0]:3d},{color[1]:3d},{color[2]:3d}): {count} pixels')
print()
# Sample some rows
for y in range(0, 160, 20):
    row = []
    for x in range(0, 240, 10):
        p = img.getpixel((x, y))
        row.append(f'{p[0]:02x}{p[1]:02x}{p[2]:02x}')
    print(f'Row {y:3d}: {" ".join(row)}')
