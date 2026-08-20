package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.ClickCounter;
import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

public class CpsModule extends HudModule {
	private final Option.Bool rightButton = new Option.Bool("right", "Rechte Taste mitzählen", true);
	private final Option.Bool label = new Option.Bool("label", "\"CPS\" anzeigen", true);

	public CpsModule() {
		super("cps", "Klicks", "Klicks pro Sekunde.", false, 0.01f, 0.10f);
		options.add(rightButton);
		options.add(label);
	}

	private String line() {
		String s = String.valueOf(ClickCounter.leftCps());
		if (rightButton.value) s += " | " + ClickCounter.rightCps();
		return label.value ? s + " CPS" : s;
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		return font.getWidth(line());
	}

	@Override
	public int height(MinecraftClient mc, TextRenderer font) {
		return font.fontHeight;
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		text(ctx, font, line(), 0, 0);
	}
}
