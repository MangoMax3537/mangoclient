package gg.mangoclient.mangoconfig;

import java.util.ArrayList;
import java.util.List;

/**
 * The PvP mods: client-side tweaks for the fight.
 *
 * Same shape as Mods - plain settings the screen draws with its usual rows,
 * and the mixins read as a field or two per frame. Nothing here changes what
 * is sent to the server; a fight plays out exactly as vanilla, it just looks
 * calmer from this side.
 */
public final class Pvp {
	private Pvp() {
	}

	public static final Option.Bool lowerShield = new Option.Bool("lowerShield", "Lower shield", false);
	public static final Option.Range lowerShieldAmount =
		new Option.Range("lowerShieldAmount", "Lower by", 40, 10, 80, "%");

	public static final Option.Bool noExplosions = new Option.Bool("noExplosions", "No explosion particles", false);
	public static final Option.Bool noTotemAnimation = new Option.Bool("noTotemAnimation", "No totem animation", false);

	static {
		lowerShield.hint = "Blocking holds the shield lower, keeping the view clear.";
		noExplosions.hint = "Crystals, TNT and wind charges stop filling the screen.";
		noTotemAnimation.hint = "Skips the totem flash across the screen.";
	}

	/** Everything that must be written to the config file. */
	public static List<Option> all() {
		List<Option> list = new ArrayList<>();
		list.add(lowerShield);
		list.add(lowerShieldAmount);
		list.add(noExplosions);
		list.add(noTotemAnimation);
		return list;
	}

	/** What the shield mixin adds to equip progress; 0 while the mod is off. */
	public static float shieldOffset() {
		return lowerShield.value ? lowerShieldAmount.value / 100f : 0f;
	}
}
