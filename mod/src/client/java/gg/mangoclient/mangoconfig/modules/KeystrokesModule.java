package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.ClickCounter;
import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.Theme;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;
import org.lwjgl.glfw.GLFW;

/**
 * WASD and the mouse buttons, lighting up as they are pressed.
 *
 * The keys are read straight from the window rather than from Minecraft's
 * bindings, so a rebound layout still shows what the fingers are doing.
 */
public class KeystrokesModule extends HudModule {
	private static final int KEY = 20;
	private static final int GAP = 2;

	private final Option.Bool showMouse = new Option.Bool("mouse", "Show mouse buttons", true);
	private final Option.Bool showSpace = new Option.Bool("space", "Show space bar", true);
	private final Option.Colour pressed = new Option.Colour("pressed", "Pressed colour", Theme.BRAND);

	public KeystrokesModule() {
		super("keystrokes", "Keys", "WASD, mouse and space.", false, 0.02f, 0.62f);
		options.add(showMouse);
		options.add(showSpace);
		options.add(pressed);
	}

	private int rows() {
		return 2 + (showMouse.value ? 1 : 0) + (showSpace.value ? 1 : 0);
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		return KEY * 3 + GAP * 2;
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return rows() * KEY + (rows() - 1) * GAP;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		int step = KEY + GAP;

		key(ctx, mc, font, "W", step, 0, KEY, GLFW.GLFW_KEY_W, preview);
		key(ctx, mc, font, "A", 0, step, KEY, GLFW.GLFW_KEY_A, preview);
		key(ctx, mc, font, "S", step, step, KEY, GLFW.GLFW_KEY_S, preview);
		key(ctx, mc, font, "D", step * 2, step, KEY, GLFW.GLFW_KEY_D, preview);

		int y = step * 2;
		if (showMouse.value) {
			int half = (KEY * 3 + GAP * 2 - GAP) / 2;
			plate(ctx, font, "LMB", 0, y, half, ClickCounter.leftCps() > 0);
			plate(ctx, font, "RMB", half + GAP, y, half, ClickCounter.rightCps() > 0);
			y += step;
		}
		if (showSpace.value) {
			boolean down = !preview && ClickCounter.isDown(mc, GLFW.GLFW_KEY_SPACE);
			plate(ctx, font, "___", 0, y, KEY * 3 + GAP * 2, down);
		}
	}

	private void key(DrawContext ctx, MinecraftClient mc, TextRenderer font,
			String label, int x, int y, int w, int glfw, boolean preview) {
		plate(ctx, font, label, x, y, w, !preview && ClickCounter.isDown(mc, glfw));
	}

	private void plate(DrawContext ctx, TextRenderer font, String label, int x, int y, int w, boolean down) {
		ctx.fill(x, y, x + w, y + KEY, down ? pressed.value : backgroundColour.value);
		int tw = font.getWidth(label);
		ctx.drawText(font, label, x + (w - tw) / 2, y + (KEY - font.fontHeight) / 2,
			down ? Theme.BRAND_FG : colour.value, shadow.value);
	}

	/** The plates are the background, so the shared one would double up. */
	@Override
	public void renderBackground(DrawContext ctx, int w, int h) {
	}
}
