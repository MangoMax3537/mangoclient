package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.registry.entry.RegistryEntry;
import net.minecraft.world.biome.Biome;

import java.util.ArrayList;
import java.util.List;

/** Where in the world the player is standing. */
public class BiomeModule extends TextModule {
	private final Option.Bool dimension = new Option.Bool("dimension", "Show the dimension", false);

	public BiomeModule() {
		super("biome", "Biome", "The biome you are standing in.", false, 0.86f, 0.10f);
		options.add(dimension);
	}

	/** "snowy_taiga" reads as an identifier; "Snowy Taiga" reads as a place. */
	private static String pretty(String id) {
		String[] words = id.replace('_', ' ').split(" ");
		StringBuilder out = new StringBuilder();
		for (String word : words) {
			if (word.isEmpty()) continue;
			if (out.length() > 0) out.append(' ');
			out.append(Character.toUpperCase(word.charAt(0))).append(word.substring(1));
		}
		return out.toString();
	}

	private String biome(MinecraftClient mc) {
		if (mc.world == null || mc.player == null) return "";
		RegistryEntry<Biome> entry = mc.world.getBiome(mc.player.getBlockPos());
		return entry.getKey().map(key -> pretty(key.getValue().getPath())).orElse("Unknown");
	}

	private String dimensionName(MinecraftClient mc) {
		if (mc.world == null) return "";
		return pretty(mc.world.getRegistryKey().getValue().getPath());
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		List<String> out = new ArrayList<>();
		String name = biome(mc);
		out.add(name.isEmpty() ? (preview ? "Plains" : "") : name);
		if (dimension.value) {
			String dim = dimensionName(mc);
			out.add(dim.isEmpty() ? (preview ? "Overworld" : "") : dim);
		}
		return out;
	}
}
