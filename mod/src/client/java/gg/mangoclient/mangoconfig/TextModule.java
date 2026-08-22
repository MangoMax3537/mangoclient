package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.List;

/**
 * A module that is just lines of text.
 *
 * Most of them are, and every one of those needs the same measuring: as wide
 * as the widest line, as tall as the lines stacked. Subclasses say what the
 * lines are and nothing else.
 */
public abstract class TextModule extends HudModule {
	protected TextModule(String id, String name, String description, boolean enabled, float x, float y) {
		super(id, name, description, enabled, x, y);
	}

	/**
	 * What to draw. In preview a module with nothing to say must still return
	 * something, or it would be a one-pixel box nobody could drag.
	 */
	protected abstract List<String> lines(MinecraftClient mc, boolean preview);

	/** The colour of a given line; overridden where a value should stand out. */
	protected int colourOf(MinecraftClient mc, int index) {
		return colour.value;
	}

	protected int lineHeight(TextRenderer font) {
		return font.fontHeight + 1;
	}

	@Override
	public boolean visible(MinecraftClient mc) {
		return !lines(mc, MangoConfig.previewing()).isEmpty();
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		int w = 0;
		for (String line : lines(mc, MangoConfig.previewing())) w = Math.max(w, font.getWidth(line));
		return Math.max(1, w);
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		int count = lines(mc, MangoConfig.previewing()).size();
		return count == 0 ? 1 : count * lineHeight(font) - 1;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		List<String> lines = lines(mc, preview);
		for (int i = 0; i < lines.size(); i++) {
			ctx.drawText(font, lines.get(i), 0, i * lineHeight(font), colourOf(mc, i), shadow.value);
		}
	}
}
