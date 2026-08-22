package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.network.ClientPlayerEntity;

import java.util.ArrayList;
import java.util.List;

/**
 * Health, hunger and armour as numbers.
 *
 * Vanilla draws them as rows of little icons, which is fine until half a heart
 * decides a fight; a number does not need counting.
 */
public class StatsModule extends TextModule {
	private final Option.Bool health = new Option.Bool("health", "Health", true);
	private final Option.Bool hunger = new Option.Bool("hunger", "Hunger", true);
	private final Option.Bool saturation = new Option.Bool("saturation", "Saturation", false);
	private final Option.Bool armour = new Option.Bool("armour", "Armour", true);
	private final Option.Choice layout = new Option.Choice("layout", "Layout", 0, "Stacked", "One line");

	public StatsModule() {
		super("stats", "Health & hunger", "Health, hunger and armour as numbers.", false, 0.5f, 0.68f);
		options.add(health);
		options.add(hunger);
		options.add(saturation);
		options.add(armour);
		options.add(layout);
	}

	/** 12.0 is noise; 12 and 12.5 are the two things worth telling apart. */
	private static String number(float value) {
		return value == Math.floor(value) ? String.valueOf((int) value) : String.format("%.1f", value);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> parts = new ArrayList<>();
		ClientPlayerEntity player = mc.player;
		if (player == null) {
			if (preview) parts.add("20 HP");
			return parts;
		}

		if (health.value) {
			float absorption = player.getAbsorptionAmount();
			String hp = number(player.getHealth()) + " HP";
			if (absorption > 0) hp += " +" + number(absorption);
			parts.add(hp);
		}
		if (hunger.value) parts.add(player.getHungerManager().getFoodLevel() + " food");
		if (saturation.value) parts.add(number(player.getHungerManager().getSaturationLevel()) + " sat");
		if (armour.value) parts.add(player.getArmor() + " armour");

		if (parts.isEmpty()) {
			if (preview) parts.add("20 HP");
			return parts;
		}
		if (layout.is("One line")) return List.of(String.join("  ", parts));
		return parts;
	}
}
