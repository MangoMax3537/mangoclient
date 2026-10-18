package gg.mangoclient.mangoconfig;

import gg.mangoclient.mangoconfig.compat.Compat;
import net.minecraft.client.gui.DrawContext;
import net.minecraft.util.Identifier;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * Custom high-resolution Lucide icons.
 *
 * Rendered from 64x64 vector-smooth textures using Compat.drawTexture.
 * Anti-aliased and crisp across all supported Minecraft versions.
 */
public final class Icons {

	public record Icon(Identifier on, Identifier off, Identifier hover) {
		public Icon(String name) {
			this(
				Identifier.of("mangoconfig", "textures/icons/" + name + "_on.png"),
				Identifier.of("mangoconfig", "textures/icons/" + name + "_off.png"),
				Identifier.of("mangoconfig", "textures/icons/" + name + "_hover.png")
			);
		}

		public void render(DrawContext ctx, int x, int y, int w, int h, boolean enabled, boolean hovered) {
			Identifier id = enabled ? on : (hovered ? hover : off);
			int size = 16;
			int ix = x + (w - size) / 2;
			int iy = y + (h - size) / 2;
			Compat.drawTexture(ctx, id, ix, iy, size, size);
		}
	}

	public static Identifier homeIcon(String name, boolean hot) {
		String key = name.toLowerCase(Locale.ROOT);
		String suffix = hot ? "_hover.png" : (key.equals("mangoconfig") ? "_on.png" : "_off.png");
		return Identifier.of("mangoconfig", "textures/icons/" + key + suffix);
	}

	private static final Map<String, Icon> ICONS = new HashMap<>();

	static {
		// HUD modules, by id
		put("fps", "fps");
		put("ping", "ping");
		put("memory", "memory");
		put("coords", "coords");
		put("direction", "direction");
		put("biome", "biome");
		put("worldtime", "worldtime");
		put("server", "server");
		put("speed", "speed");
		put("armour", "armour");
		put("stats", "stats");
		put("saturation", "saturation");
		put("potions", "potions");
		put("items", "items");
		put("cps", "cps");
		put("reach", "reach");
		put("combo", "combo");
		put("keystrokes", "keystrokes");
		put("movement", "movement");
		put("sprint", "movement");
		put("session", "session");
		put("clock", "clock");

		// Standalone mod cards, by name
		put("Zoom", "zoom");
		put("Crosshair", "crosshair");
		put("No Hurt Cam", "no_hurt_cam");
		put("Fullbright", "fullbright");
		put("Mango Badge", "mango_badge");

		put("Lower Shield", "lower_shield");
		put("No Explosions", "no_explosions");
		put("No Totem Pop", "no_totem_pop");

		put("Block Overlay", "block_overlay");
		put("Brightness", "brightness");
		put("Colour Saturation", "colour_saturation");
		put("Custom Chat", "custom_chat");
		put("Custom F3", "custom_f3");
		put("Custom Fog", "custom_fog");
		put("Hitboxes", "hitboxes");
		put("Tab & Scoreboard", "tab_scoreboard");
		put("UI Scale", "ui_scale");

		put("Hide Scoreboard", "hide_scoreboard");
		put("Hide Boss Bar", "hide_boss_bar");
		put("Hide Effects", "hide_effects");
		put("Hide Portal", "hide_portal");
		put("Hide Nausea", "hide_nausea");
		put("Hide Item Name", "hide_item_name");
	}

	private Icons() {
	}

	private static void put(String key, String iconName) {
		ICONS.put(key.toLowerCase(Locale.ROOT), new Icon(iconName));
	}

	public static Icon of(String key) {
		return ICONS.get(key.toLowerCase(Locale.ROOT));
	}
}
