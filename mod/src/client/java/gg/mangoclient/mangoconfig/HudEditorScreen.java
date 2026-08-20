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
 * hand-aligning a corner by pixel is nobody's idea of a good time.
 */
public class HudEditorScreen extends Screen {
	/** How close to an edge, in pixels, before it sticks. */
	private static final int SNAP = 6;

	private final Screen parent;

	private HudModule dragging;
	private int grabX;
	private int grabY;

	public HudEditorScreen(Screen parent) {
		super(Text.literal("HUD anordnen"));
		this.parent = parent;
	}

	@Override
	public void render(DrawContext ctx, int mouseX, int mouseY, float delta) {
		MinecraftClient mc = MinecraftClient.getInstance();
		ctx.fill(0, 0, this.width, this.height, 0x66000000);

		MangoConfig.renderModules(ctx, mc, true);

		for (HudModule module : MangoConfig.modules()) {
			int[] b = MangoConfig.bounds(mc, module);
			boolean hot = module == dragging
				|| (mouseX >= b[0] && mouseX < b[0] + b[2] && mouseY >= b[1] && mouseY < b[1] + b[3]);
			int colour = module.enabled ? (hot ? Theme.BRAND_HOVER : Theme.BRAND) : Theme.SURFACE_4;
			outline(ctx, b[0], b[1], b[2], b[3], colour);
		}

		String hint = "Ziehen zum Verschieben · Esc zurück";
		int w = this.textRenderer.getWidth(hint);
		int hx = (this.width - w) / 2;
		ctx.fill(hx - 6, this.height - 24, hx + w + 6, this.height - 10, Theme.SURFACE_2);
		ctx.drawText(this.textRenderer, hint, hx, this.height - 20, Theme.TEXT_2, false);

		super.render(ctx, mouseX, mouseY, delta);
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
		List<HudModule> all = MangoConfig.modules();

		// Backwards, so the module drawn last is the one grabbed first.
		for (int i = all.size() - 1; i >= 0; i--) {
			HudModule module = all.get(i);
			int[] b = MangoConfig.bounds(mc, module);
			if (click.x() >= b[0] && click.x() < b[0] + b[2] && click.y() >= b[1] && click.y() < b[1] + b[3]) {
				dragging = module;
				grabX = (int) click.x() - b[0];
				grabY = (int) click.y() - b[1];
				return true;
			}
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
