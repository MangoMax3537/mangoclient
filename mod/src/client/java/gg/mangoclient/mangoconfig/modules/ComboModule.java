package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Combat;
import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;

import java.util.List;

/**
 * Hits landed on the same target without a break.
 *
 * The count lapses on its own once the fight stops, so there is nothing to
 * reset and nothing to get stuck showing a number from ten minutes ago.
 */
public class ComboModule extends TextModule {
	private final Option.Range showFrom = new Option.Range("showFrom", "Show from", 2, 1, 10, " hits");
	private final Option.Bool label = new Option.Bool("label", "Show \"hits\"", false);
	private final Option.Bool escalate = new Option.Bool("escalate", "Colour as it climbs", true);

	public ComboModule() {
		super("combo", "Combo", "Hits landed without a break.", false, 0.5f, 0.55f);
		options.add(showFrom);
		options.add(label);
		options.add(escalate);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		int combo = Combat.combo();
		if (combo < showFrom.value) {
			return preview ? List.of(text(showFrom.value)) : List.of();
		}
		return List.of(text(combo));
	}

	private String text(int combo) {
		return label.value ? combo + " hits" : combo + "x";
	}

	@Override
	protected int colourOf(MinecraftClient mc, int index) {
		if (!escalate.value) return colour.value;
		int combo = Math.max(Combat.combo(), showFrom.value);
		if (combo >= 10) return 0xFFFF4B4B;
		if (combo >= 5) return 0xFFF2A53C;
		return 0xFF1BD96A;
	}
}
