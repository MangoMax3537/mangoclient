package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

public class FpsModule extends HudModule {
	private final Option.Bool label = new Option.Bool("label", "\"FPS\" anzeigen", true);

	public FpsModule() {
		super("fps", "FPS", "Bilder pro Sekunde.", true, 0.01f, 0.02f);
		options.add(label);
	}

	private String line(MinecraftClient mc) {
		int fps = mc.getCurrentFps();
		return label.value ? fps + " FPS" : String.valueOf(fps);
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		return font.getWidth(line(mc));
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return font.fontHeight;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		text(ctx, font, line(mc), 0, 0);
	}
}
