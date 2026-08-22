package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;

import java.util.List;

/**
 * How much of the memory the game was given is in use.
 *
 * Worth watching on a modded instance: a client that keeps hitting its ceiling
 * stutters, and the number is the first thing anyone asks for.
 */
public class MemoryModule extends TextModule {
	private final Option.Bool percent = new Option.Bool("percent", "Show percentage", true);
	private final Option.Bool total = new Option.Bool("total", "Show the total", true);
	private final Option.Bool warn = new Option.Bool("warn", "Colour when nearly full", true);

	public MemoryModule() {
		super("memory", "Memory", "How much RAM the game is using.", false, 0.01f, 0.18f);
		options.add(percent);
		options.add(total);
		options.add(warn);
	}

	private static double used() {
		Runtime rt = Runtime.getRuntime();
		return (rt.totalMemory() - rt.freeMemory()) / 1073741824.0;
	}

	private static double max() {
		return Runtime.getRuntime().maxMemory() / 1073741824.0;
	}

	private static float fraction() {
		double limit = max();
		return limit <= 0 ? 0f : (float) (used() / limit);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		StringBuilder line = new StringBuilder(String.format("%.1f", used()));
		if (total.value) line.append(" / ").append(String.format("%.1f", max()));
		line.append(" GB");
		if (percent.value) line.append(String.format("  %d%%", Math.round(fraction() * 100)));
		return List.of(line.toString());
	}

	@Override
	protected int colourOf(MinecraftClient mc, int index) {
		if (!warn.value) return colour.value;
		float f = fraction();
		if (f >= 0.9f) return 0xFFFF4B4B;
		if (f >= 0.75f) return 0xFFF2A53C;
		return colour.value;
	}
}
