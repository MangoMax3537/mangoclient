package gg.mangoclient.mangoconfig;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import com.google.gson.JsonObject;
import net.fabricmc.loader.api.FabricLoader;
import org.lwjgl.glfw.GLFW;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

/**
 * The settings on disk, at config/mangoconfig.json.
 *
 * Written whenever something changes rather than only on quit, because a
 * crash should not cost the player the layout they just arranged.
 */
public class ConfigFile {
	private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();

	public boolean hudEnabled = true;
	public int openKey = GLFW.GLFW_KEY_RIGHT_SHIFT;

	private final Path file;

	public ConfigFile() {
		this.file = FabricLoader.getInstance().getConfigDir().resolve("mangoconfig.json");
	}

	public void load(List<HudModule> modules) {
		JsonObject root;
		try {
			if (!Files.exists(file)) return;
			root = GSON.fromJson(Files.readString(file), JsonObject.class);
		} catch (Exception e) {
			return; // a corrupt file is not worth crashing a game session over
		}
		if (root == null) return;

		if (root.has("hudEnabled")) hudEnabled = root.get("hudEnabled").getAsBoolean();
		if (root.has("openKey")) openKey = root.get("openKey").getAsInt();

		JsonObject mods = root.getAsJsonObject("modules");
		if (mods == null) return;
		for (HudModule module : modules) {
			JsonObject own = mods.getAsJsonObject(module.id);
			if (own != null) module.load(own);
		}
	}

	public void save(List<HudModule> modules) {
		JsonObject root = new JsonObject();
		root.addProperty("hudEnabled", hudEnabled);
		root.addProperty("openKey", openKey);

		JsonObject mods = new JsonObject();
		for (HudModule module : modules) {
			JsonObject own = new JsonObject();
			module.save(own);
			mods.add(module.id, own);
		}
		root.add("modules", mods);

		try {
			Files.createDirectories(file.getParent());
			Files.writeString(file, GSON.toJson(root));
		} catch (IOException e) {
			// Nothing useful to do about it in game; the settings stay in memory.
		}
	}
}
