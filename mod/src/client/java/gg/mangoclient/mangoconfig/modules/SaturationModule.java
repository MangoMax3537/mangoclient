package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.entity.player.HungerManager;

import java.util.ArrayList;
import java.util.List;

/**
 * The hunger numbers the bar hides: saturation drains invisibly, and the
 * moment it runs out is the moment sprinting starts eating food.
 */
public class SaturationModule extends TextModule {
	private final Option.Bool label = new Option.Bool("label", "Show labels", true);
	private final Option.Bool food = new Option.Bool("food", "Show food level", false);

	public SaturationModule() {
		super("saturation", "Saturation", "The hidden saturation behind the hunger bar.", false, 0.5f, 0.9f);
		options.add(label);
		options.add(food);
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		if (mc.player == null) {
			if (preview) out.add(label.value ? "Saturation: 5.0" : "5.0");
			return out;
		}
		HungerManager hunger = mc.player.getHungerManager();
		String value = String.format("%.1f", hunger.getSaturationLevel());
		out.add(label.value ? "Saturation: " + value : value);
		if (food.value) {
			out.add(label.value ? "Food: " + hunger.getFoodLevel() : String.valueOf(hunger.getFoodLevel()));
		}
		return out;
	}
}
