package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.HudModule;
import gg.mangoclient.mangoconfig.Option;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.time.LocalTime;
import java.time.format.DateTimeFormatter;

public class ClockModule extends HudModule {
	private static final DateTimeFormatter WITH_SECONDS = DateTimeFormatter.ofPattern("HH:mm:ss");
	private static final DateTimeFormatter WITHOUT = DateTimeFormatter.ofPattern("HH:mm");

	private final Option.Bool seconds = new Option.Bool("seconds", "Show seconds", false);

	public ClockModule() {
		super("clock", "Clock", "The real-world time.", false, 0.90f, 0.02f);
		options.add(seconds);
	}

	private String line() {
		return LocalTime.now().format(seconds.value ? WITH_SECONDS : WITHOUT);
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
