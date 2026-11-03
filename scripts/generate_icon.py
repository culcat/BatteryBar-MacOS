import math
from PIL import Image, ImageDraw, ImageFilter

SIZE = 1024
SCALE = 2  # Super-sample 2x for ultra smooth anti-aliasing (2048 -> 1024)
CANVAS_SIZE = SIZE * SCALE

def create_icon():
    # Base 32-bit RGBA canvas
    img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    
    # 1. macOS Squircle dimensions
    margin = int(100 * SCALE)
    squircle_size = CANVAS_SIZE - (2 * margin)
    radius = int(185 * SCALE)
    
    # Create squircle mask
    mask = Image.new("L", (CANVAS_SIZE, CANVAS_SIZE), 0)
    draw_mask = ImageDraw.Draw(mask)
    draw_mask.rounded_rectangle(
        [margin, margin, margin + squircle_size, margin + squircle_size],
        radius=radius,
        fill=255
    )
    
    # 2. Draw Squircle Drop Shadow
    shadow = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_shadow = ImageDraw.Draw(shadow)
    shadow_offset = int(24 * SCALE)
    draw_shadow.rounded_rectangle(
        [margin, margin + shadow_offset, margin + squircle_size, margin + squircle_size + shadow_offset],
        radius=radius,
        fill=(0, 0, 0, 160)
    )
    shadow = shadow.filter(ImageFilter.GaussianBlur(int(28 * SCALE)))
    img.alpha_composite(shadow)
    
    # 3. Squircle Background Gradient (Deep Space Navy to Pitch Dark)
    sq_bg = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    for y in range(margin, margin + squircle_size):
        t = (y - margin) / squircle_size
        # #1c222e -> #0b0e14
        r = int(28 * (1 - t) + 11 * t)
        g = int(34 * (1 - t) + 14 * t)
        b = int(46 * (1 - t) + 20 * t)
        for x in range(margin, margin + squircle_size):
            sq_bg.putpixel((x, y), (r, g, b, 255))
            
    # Apply squircle mask to background
    sq_bg.putalpha(mask)
    
    # 4. Ambient Radial Emerald Glow behind the battery
    glow_canvas = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_glow = ImageDraw.Draw(glow_canvas)
    cx, cy = CANVAS_SIZE // 2, int(530 * SCALE)
    glow_r = int(320 * SCALE)
    draw_glow.ellipse(
        [cx - glow_r, cy - glow_r, cx + glow_r, cy + glow_r],
        fill=(16, 185, 129, 90)
    )
    glow_canvas = glow_canvas.filter(ImageFilter.GaussianBlur(int(70 * SCALE)))
    # Composite glow inside squircle
    sq_bg = Image.composite(glow_canvas, sq_bg, mask)
    img.alpha_composite(sq_bg)
    
    # 5. Squircle Border Chamfer (Subtle 1.5px bevel)
    border_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_border = ImageDraw.Draw(border_img)
    draw_border.rounded_rectangle(
        [margin, margin, margin + squircle_size, margin + squircle_size],
        radius=radius,
        outline=(255, 255, 255, 60),
        width=int(2.5 * SCALE)
    )
    img.alpha_composite(border_img)
    
    # 6. Battery Shell Coordinates
    b_w = int(410 * SCALE)
    b_h = int(540 * SCALE)
    b_x1 = (CANVAS_SIZE - b_w) // 2
    b_y1 = int(256 * SCALE)
    b_x2 = b_x1 + b_w
    b_y2 = b_y1 + b_h
    b_rad = int(60 * SCALE)
    
    # Battery Terminal Cap (Top)
    cap_w = int(180 * SCALE)
    cap_h = int(36 * SCALE)
    cap_x1 = (CANVAS_SIZE - cap_w) // 2
    cap_y1 = b_y1 - cap_h + int(4 * SCALE)
    cap_x2 = cap_x1 + cap_w
    cap_y2 = b_y1 + int(4 * SCALE)
    
    cap_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_cap = ImageDraw.Draw(cap_img)
    draw_cap.rounded_rectangle(
        [cap_x1, cap_y1, cap_x2, cap_y2],
        radius=int(14 * SCALE),
        fill=(203, 213, 225, 255),
        outline=(255, 255, 255, 120),
        width=int(2 * SCALE)
    )
    img.alpha_composite(cap_img)
    
    # Battery Shell (Dark Glass Housing)
    glass_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_glass = ImageDraw.Draw(glass_img)
    draw_glass.rounded_rectangle(
        [b_x1, b_y1, b_x2, b_y2],
        radius=b_rad,
        fill=(10, 14, 22, 230),
        outline=(255, 255, 255, 75),
        width=int(4 * SCALE)
    )
    img.alpha_composite(glass_img)
    
    # 7. Liquid Energy Fill (~75% charged)
    pad = int(14 * SCALE)
    inner_x1 = b_x1 + pad
    inner_y1 = b_y1 + pad
    inner_x2 = b_x2 - pad
    inner_y2 = b_y2 - pad
    inner_rad = b_rad - pad
    
    inner_mask = Image.new("L", (CANVAS_SIZE, CANVAS_SIZE), 0)
    draw_inner_mask = ImageDraw.Draw(inner_mask)
    draw_inner_mask.rounded_rectangle(
        [inner_x1, inner_y1, inner_x2, inner_y2],
        radius=inner_rad,
        fill=255
    )
    
    # Fluid height: 75%
    fluid_top = inner_y1 + int((inner_y2 - inner_y1) * 0.28)
    fluid_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    for y in range(fluid_top, inner_y2):
        t = (y - fluid_top) / max(1, (inner_y2 - fluid_top))
        # Gradient: Emerald #10B981 -> Cyan/Teal #059669 -> Deep Forest Green
        r = int(52 * (1 - t) + 4 * t)
        g = int(211 * (1 - t) + 120 * t)
        b = int(153 * (1 - t) + 87 * t)
        for x in range(inner_x1, inner_x2):
            fluid_img.putpixel((x, y), (r, g, b, 255))
            
    # Glowing surface meniscus
    draw_fluid = ImageDraw.Draw(fluid_img)
    draw_fluid.ellipse(
        [inner_x1, fluid_top - int(10 * SCALE), inner_x2, fluid_top + int(10 * SCALE)],
        fill=(167, 243, 208, 250)
    )
    
    # Apply inner mask
    fluid_clipped = Image.composite(fluid_img, Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0)), inner_mask)
    img.alpha_composite(fluid_clipped)
    
    # 8. Measurement Tick Marks
    tick_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_ticks = ImageDraw.Draw(tick_img)
    t_len = int(18 * SCALE)
    for frac in [0.25, 0.50, 0.75]:
        ty = inner_y1 + int((inner_y2 - inner_y1) * (1.0 - frac))
        draw_ticks.line([inner_x1 + int(8*SCALE), ty, inner_x1 + int(8*SCALE) + t_len, ty], fill=(255, 255, 255, 70), width=int(3*SCALE))
        draw_ticks.line([inner_x2 - int(8*SCALE) - t_len, ty, inner_x2 - int(8*SCALE), ty], fill=(255, 255, 255, 70), width=int(3*SCALE))
    img.alpha_composite(tick_img)
    
    # 9. Center Lightning Bolt (Electric Icon)
    bolt_points = [
        (int(536 * SCALE), int(360 * SCALE)),
        (int(450 * SCALE), int(495 * SCALE)),
        (int(515 * SCALE), int(495 * SCALE)),
        (int(475 * SCALE), int(645 * SCALE)),
        (int(580 * SCALE), int(480 * SCALE)),
        (int(518 * SCALE), int(480 * SCALE)),
    ]
    
    # Bolt Shadow & Glow
    bolt_glow = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_bolt_glow = ImageDraw.Draw(bolt_glow)
    draw_bolt_glow.polygon(bolt_points, fill=(16, 185, 129, 200))
    bolt_glow = bolt_glow.filter(ImageFilter.GaussianBlur(int(14 * SCALE)))
    img.alpha_composite(bolt_glow)
    
    # Bolt Core
    bolt_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_bolt = ImageDraw.Draw(bolt_img)
    draw_bolt.polygon(bolt_points, fill=(255, 255, 255, 255))
    img.alpha_composite(bolt_img)
    
    # 10. Specular Reflection Sheen (Gloss)
    gloss_img = Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0))
    draw_gloss = ImageDraw.Draw(gloss_img)
    gloss_poly = [
        (inner_x1 + int(20 * SCALE), inner_y1),
        (inner_x1 + int(100 * SCALE), inner_y1),
        (inner_x1 + int(50 * SCALE), inner_y2),
        (inner_x1 + int(15 * SCALE), inner_y2)
    ]
    draw_gloss.polygon(gloss_poly, fill=(255, 255, 255, 25))
    gloss_clipped = Image.composite(gloss_img, Image.new("RGBA", (CANVAS_SIZE, CANVAS_SIZE), (0, 0, 0, 0)), inner_mask)
    img.alpha_composite(gloss_clipped)
    
    # Resize down with Lanczos anti-aliasing
    final_icon = img.resize((SIZE, SIZE), Image.Resampling.LANCZOS)
    return final_icon

if __name__ == "__main__":
    icon = create_icon()
    icon.save("/Users/aleksandr/.gemini/antigravity/scratch/battery-bar/assets/logo.png", "PNG")
    print("Successfully generated assets/logo.png (1024x1024)!")
