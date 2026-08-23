package gg.mangoclient.mangoconfig;

import net.minecraft.item.Item;
import net.minecraft.item.ItemStack;
import net.minecraft.item.Items;
import net.minecraft.util.Identifier;

import java.util.HashMap;
import java.util.Locale;
import java.util.Map;

/**
 * The face of each mod card: a Minecraft item, or one of the game's own GUI
 * sprites where no item fits (there is no "ping" item, but there are the
 * server list's ping bars).
 *
 * Looked up by module id or card name, so the modules and ModEntry know
 * nothing about rendering; a card without an entry here falls back to its
 * initial on a plate, same as before.
 */
public final class Icons {
	/** An item to draw, or a GUI sprite at its native size when set. */
	public record Icon(ItemStack stack, Identifier sprite, int width, int height) {
		static Icon item(Item item) {
			return new Icon(new ItemStack(item), null, 16, 16);
		}

		static Icon sprite(String id, int width, int height) {
			return new Icon(null, Identifier.ofVanilla(id), width, height);
		}
	}

	private static final Map<String, Icon> ICONS = new HashMap<>();

	static {
		// HUD modules, by id.
		put("fps", Icon.item(Items.REDSTONE));
		put("ping", Icon.sprite("icon/ping_5", 10, 8));
		put("memory", Icon.item(Items.COMPARATOR));
		put("coords", Icon.item(Items.COMPASS));
		put("direction", Icon.item(Items.RECOVERY_COMPASS));
		put("biome", Icon.item(Items.OAK_SAPLING));
		put("worldtime", Icon.item(Items.DAYLIGHT_DETECTOR));
		put("server", Icon.sprite("icon/ping_5", 10, 8));
		put("speed", Icon.item(Items.SUGAR));
		put("armour", Icon.item(Items.DIAMOND_CHESTPLATE));
		put("stats", Icon.item(Items.GOLDEN_APPLE));
		put("saturation", Icon.item(Items.COOKED_BEEF));
		put("potions", Icon.item(Items.BREWING_STAND));
		put("items", Icon.item(Items.CHEST));
		put("cps", Icon.item(Items.STONE_BUTTON));
		put("reach", Icon.item(Items.FISHING_ROD));
		put("combo", Icon.item(Items.BLAZE_POWDER));
		put("keystrokes", Icon.item(Items.LEVER));
		put("movement", Icon.item(Items.FEATHER));
		put("session", Icon.item(Items.WRITABLE_BOOK));
		put("clock", Icon.item(Items.CLOCK));

		// Standalone mod cards, by name.
		put("Zoom", Icon.item(Items.SPYGLASS));
		put("Crosshair", Icon.sprite("hud/crosshair", 15, 15));
		put("No Hurt Cam", Icon.item(Items.CACTUS));
		put("Fullbright", Icon.item(Items.GLOWSTONE));
		put("Mango Badge", Icon.item(Items.PLAYER_HEAD));

		put("Lower Shield", Icon.item(Items.SHIELD));
		put("No Explosions", Icon.item(Items.END_CRYSTAL));
		put("No Totem Pop", Icon.item(Items.TOTEM_OF_UNDYING));

		put("Hide Scoreboard", Icon.item(Items.OAK_SIGN));
		put("Hide Boss Bar", Icon.item(Items.DRAGON_HEAD));
		put("Hide Effects", Icon.item(Items.BEACON));
		put("Hide Portal", Icon.item(Items.OBSIDIAN));
		put("Hide Nausea", Icon.item(Items.PUFFERFISH));
		put("Hide Item Name", Icon.item(Items.NAME_TAG));
	}

	private Icons() {
	}

	private static void put(String key, Icon icon) {
		ICONS.put(key.toLowerCase(Locale.ROOT), icon);
	}

	/** Null when no icon is defined; the card then shows its initial. */
	public static Icon of(String key) {
		return ICONS.get(key.toLowerCase(Locale.ROOT));
	}
}
