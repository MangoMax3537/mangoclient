package gg.mangoclient.mangoconfig;

import net.minecraft.client.MinecraftClient;
import net.minecraft.resource.Resource;
import net.minecraft.util.Identifier;

import java.io.InputStream;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.function.Predicate;

/**
 * The assets this jar brings for itself, handed to Minecraft directly.
 *
 * A mod's own assets normally reach the game through Fabric API's resource
 * loader, and this mod runs without Fabric API. So the files are read from
 * the jar on the classpath and offered whenever the game looks for one under
 * the mangoconfig namespace and nothing else has it - which is also what a
 * resource pack overriding one of them still gets to do first.
 */
public final class BundledAssets {
	private BundledAssets() {
	}

	/** Every folder whose files are found by listing rather than by name. */
	private static final String[] LISTED = {"post_effect", "shaders/post"};

	private static Map<String, Map<Identifier, Resource>> listed;

	private static boolean exists(String path) {
		return BundledAssets.class.getResource(path) != null;
	}

	private static String file(Identifier id) {
		return "/assets/" + id.getNamespace() + "/" + id.getPath();
	}

	private static Resource resource(Identifier id) {
		String path = file(id);
		return new Resource(MinecraftClient.getInstance().getDefaultResourcePack(), () -> {
			InputStream in = BundledAssets.class.getResourceAsStream(path);
			if (in == null) throw new java.io.FileNotFoundException(path);
			return in;
		});
	}

	public static Optional<Resource> get(Identifier id) {
		if (!id.getNamespace().equals("mangoconfig") || !exists(file(id))) return Optional.empty();
		return Optional.of(resource(id));
	}

	/**
	 * What the jar holds below a listed folder. The saturation chains are the
	 * only listed files, and their names are known, so no jar walking is needed.
	 */
	public static Map<Identifier, Resource> find(String folder, Predicate<Identifier> filter) {
		if (listed == null) {
			Map<String, Map<Identifier, Resource>> all = new LinkedHashMap<>();
			for (String dir : LISTED) {
				Map<Identifier, Resource> found = new LinkedHashMap<>();
				for (int step = 0; step <= 200; step += 10) {
					Identifier id = Identifier.of("mangoconfig",
						dir + "/saturation_" + String.format(java.util.Locale.ROOT, "%03d", step) + ".json");
					if (exists(file(id))) found.put(id, resource(id));
				}
				all.put(dir, found);
			}
			listed = all;
		}
		Map<Identifier, Resource> found = listed.get(folder);
		if (found == null || found.isEmpty()) return Map.of();
		Map<Identifier, Resource> out = new LinkedHashMap<>();
		found.forEach((id, res) -> {
			if (filter.test(id)) out.put(id, res);
		});
		return out;
	}

	/** Whether the post chain behind a saturation id is really in the jar. */
	public static boolean hasChain(Identifier id) {
		String path = id.getPath();
		return exists("/assets/" + id.getNamespace() + "/" + (path.endsWith(".json") ? path : "post_effect/" + path + ".json"));
	}
}
