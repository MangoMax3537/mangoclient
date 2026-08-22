package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.gui.Click;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.client.gui.screen.Screen;
import net.minecraft.client.input.KeyInput;
import net.minecraft.text.Text;
import org.lwjgl.glfw.GLFW;

import java.util.List;

/**
 * Drag the HUD into place.
 *
 * Every enabled module is drawn where it really sits, with a frame around it,
 * so what is arranged here is exactly what shows up in game. Edges snap, since
 * hand-aligning a corner by pixel is nobody's idea of a good time. Hovering a
 * module offers a small settings button straight into its options, and the
 * scroll wheel resizes it on the spot.
 */
public class HudEditorScreen extends Screen {
	/** How close to an edge, in pixels, before it sticks. */
	private static final int SNAP = 6;
	/** The settings button hovering over a module's corner. */
	private static final int GEAR = 12;

	private final Screen parent;

	private HudModule dragging;
	private int grabX;
	private int grabY;

	public HudEditorScreen(Screen parent) {
		super(Text.literal("Arrange HUD"));
		this.parent = parent;
	}

	/** The module under the mouse; the one drawn last wins, as when dragging. */
	private HudModule hovered(MinecraftClient mc, double mx, double my) {
		List<HudModule> all = MangoConfig.modules();
		for (int i = all.size() - 1; i >= 0; i--) {
			HudModule module = all.get(i);
			if (!module.enabled) continue;
			int[] b = MangoConfig.bounds(mc, module);
			if (inside(mx, my, b) || inside(mx, my, gearRect(b))) return module;
		}
		return null;
	}

	/** Above the frame's right corner, or below it when there is no room. */
	private int[] gearRect(int[] b) {
		int x = Math.max(2, Math.min(this.width - GEAR - 2, b[0] + b[2] - GEAR));
		int y = b[1] - GEAR - 2;
		if (y < 2) y = b[1] + b[3] + 2;
		return new int[]{x, y, GEAR, GEAR};
	}

	private static boolean inside(double mx, double my, int[] r) {
		return mx >= r[0] && mx < r[0] + r[2] && my >= r[1] && my < r[1] + r[3];
	}

	@Override
	public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ctx.fill(0, 0, this.width, this.height, 0x66000000);

		MangoConfig.renderModules(ctx, mc, true);

		HudModule hover = dragging != null ? dragging : hovered(mc, mouseX, mouseY);
		for (HudModule module : MangoConfig.modules()) {
			if (!module.enabled) continue;
			int[] b = MangoConfig.bounds(mc, module);
			boolean hot = module == hover;
			outline(ctx, b[0], b[1], b[2], b[3], hot ? Theme.BRAND_HOVER : Theme.BRAND);
			if (hot && dragging == null) drawGear(ctx, gearRect(b), mouseX, mouseY);
		}

		String hint = hover != null
			? hover.name + " · " + hover.scale.value + "% · Scroll to resize"
			: "Drag to move · Scroll to resize · Esc to go back";
		int w = this.textRenderer.getWidth(hint);
		int hx = (this.width - w) / 2;
		ctx.fill(hx - 6, this.height - 24, hx + w + 6, this.height - 10, Theme.SURFACE_2);
		ctx.drawText(this.textRenderer, hint, hx, this.height - 20, Theme.TEXT_2, false);

		super.render(ctx, mouseX, mouseY, delta);
	}

	/** A tiny sliders icon drawn from fills, so no font has to carry a gear. */
	private void drawGear(DrawContext ctx, int[] r, int mouseX, int mouseY) {
		boolean hot = inside(mouseX, mouseY, r);
		ctx.fill(r[0] - 1, r[1] - 1, r[0] + r[2] + 1, r[1] + r[3] + 1, hot ? Theme.BRAND : Theme.SURFACE_4);
		ctx.fill(r[0], r[1], r[0] + r[2], r[1] + r[3], Theme.SURFACE_2);
		int x = r[0] + 2;
		int w = r[2] - 4;
		int fg = hot ? Theme.BRAND_HOVER : Theme.TEXT_2;
		for (int i = 0; i < 3; i++) {
			int y = r[1] + 3 + i * 3;
			ctx.fill(x, y, x + w, y + 1, fg);
			int knob = x + (i == 1 ? w - 3 : i == 0 ? 1 : 3);
			ctx.fill(knob, y - 1, knob + 2, y + 2, fg);
		}
	}

	private void outline(DrawContext ctx, int x, int y, int w, int h, int colour) {
		ctx.fill(x, y, x + w, y + 1, colour);
		ctx.fill(x, y + h - 1, x + w, y + h, colour);
		ctx.fill(x, y, x + 1, y + h, colour);
		ctx.fill(x + w - 1, y, x + w, y + h, colour);
	}

	@Override
	public boolean mouseClicked(Click click, boolean doubled) {
		MinecraftClient mc = MinecraftClient.getInstance();

		// The gear outranks a drag: it may hang over a neighbouring module.
		HudModule hover = hovered(mc, click.x(), click.y());
		if (hover != null && inside(click.x(), click.y(), gearRect(MangoConfig.bounds(mc, hover)))) {
			MangoConfig.save();
			mc.setScreen(new MangoConfigScreen(hover));
			return true;
		}

		if (hover != null && inside(click.x(), click.y(), MangoConfig.bounds(mc, hover))) {
			int[] b = MangoConfig.bounds(mc, hover);
			dragging = hover;
			grabX = (int) click.x() - b[0];
			grabY = (int) click.y() - b[1];
			return true;
		}
		return super.mouseClicked(click, doubled);
	}

	@Override
	public boolean mouseDragged(Click click, double dx, double dy) {
		if (dragging == null) return super.mouseDragged(click, dx, dy);

		MinecraftClient mc = MinecraftClient.getInstance();
		int[] b = MangoConfig.bounds(mc, dragging);
		int screenW = mc.getWindow().getScaledWidth();
		int screenH = mc.getWindow().getScaledHeight();

		int px = (int) click.x() - grabX;
		int py = (int) click.y() - grabY;

		px = snap(px, b[2], screenW);
		py = snap(py, b[3], screenH);

		dragging.x = clamp01(px / (float) screenW);
		dragging.y = clamp01(py / (float) screenH);
		return true;
	}

	/** The wheel resizes whatever is under the mouse, 5% a notch. */
	@Override
	public boolean mouseScrolled(double mouseX, double mouseY, double horizontal, double vertical) {
		HudModule hover = dragging != null ? dragging
			: hovered(MinecraftClient.getInstance(), mouseX, mouseY);
		if (hover == null) return super.mouseScrolled(mouseX, mouseY, horizontal, vertical);
		hover.scale.set(hover.scale.value + (vertical > 0 ? 5 : -5));
		return true;
	}

	/** Stick to either edge, or to the middle, when close enough. */
	private static int snap(int pos, int size, int screen) {
		if (Math.abs(pos) <= SNAP) return 0;
		if (Math.abs(pos + size - screen) <= SNAP) return screen - size;
		int centre = (screen - size) / 2;
		if (Math.abs(pos - centre) <= SNAP) return centre;
		return Math.max(0, Math.min(screen - size, pos));
	}

	private static float clamp01(float v) {
		return Math.max(0f, Math.min(1f, v));
	}

	@Override
	public boolean mouseReleased(Click click) {
		if (dragging != null) {
			dragging = null;
			MangoConfig.save();
			return true;
		}
		return super.mouseReleased(click);
	}

	@Override
	public boolean keyPressed(KeyInput input) {
		if (input.key() == GLFW.GLFW_KEY_ESCAPE) {
			this.close();
			return true;
		}
		return super.keyPressed(input);
	}

	@Override
	public void close() {
		MangoConfig.save();
		MinecraftClient.getInstance().setScreen(parent);
	}
}
