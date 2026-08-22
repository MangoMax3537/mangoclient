package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;

import java.util.ArrayList;
import java.util.List;

/**
 * What the player is currently doing: sprinting, sneaking, flying.
 *
 * Worth having with toggle sprint switched on, where there is otherwise no way
 * to tell whether the toggle took.
 */
public class SprintModule extends TextModule {
	private final Option.Bool sprint = new Option.Bool("sprint", "Sprinting", true);
	private final Option.Bool sneak = new Option.Bool("sneak", "Sneaking", true);
	private final Option.Bool fly = new Option.Bool("fly", "Flying", true);
	private final Option.Bool brackets = new Option.Bool("brackets", "In brackets", true);

	public SprintModule() {
		super("movement", "Movement", "Sprinting, sneaking and flying.", false, 0.5f, 0.75f);
		options.add(sprint);
		options.add(sneak);
		options.add(fly);
		options.add(brackets);
	}

	private String wrap(String state) {
		return brackets.value ? "[" + state + "]" : state;
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		if (mc.player == null) {
			if (preview) out.add(wrap("Sprinting"));
			return out;
		}
		if (fly.value && mc.player.getAbilities().flying) out.add(wrap("Flying"));
		if (sprint.value && mc.player.isSprinting()) out.add(wrap("Sprinting"));
		if (sneak.value && mc.player.isSneaking()) out.add(wrap("Sneaking"));
		if (out.isEmpty() && preview) out.add(wrap("Sprinting"));
		return out;
	}
}
