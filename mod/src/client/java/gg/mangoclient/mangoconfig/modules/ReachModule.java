package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Combat;
import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;

import java.util.List;

/**
 * How far away the last hit landed.
 *
 * The reading is held for a few seconds after the swing, because a number that
 * vanishes the instant the fight pauses is one nobody ever reads.
 */
public class ReachModule extends TextModule {
	private final Option.Bool label = new Option.Bool("label", "Show \"m\"", true);
	private final Option.Bool hideIdle = new Option.Bool("hideIdle", "Hide between fights", false);

	public ReachModule() {
		super("reach", "Reach", "How far away your last hit landed.", false, 0.5f, 0.60f);
		options.add(label);
		options.add(hideIdle);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		if (!Combat.hasReach()) {
			if (preview) return List.of(label.value ? "3.00 m" : "3.00");
			if (hideIdle.value) return List.of();
			return List.of(label.value ? "-- m" : "--");
		}
		String value = String.format("%.2f", Combat.reach());
		return List.of(label.value ? value + " m" : value);
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		return font.getWidth(label.value ? "8.88 m" : "8.88");
	}
}
