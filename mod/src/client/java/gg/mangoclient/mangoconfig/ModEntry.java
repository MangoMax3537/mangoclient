package gg.mangoclient.mangoconfig;

import java.util.ArrayList;
import java.util.List;

import java.util.Locale;

/**
 * One card on the mods grid.
 *
 * A HUD module and a standalone mod (zoom, say) are different things underneath
 * - one is drawn on screen and owns a position, the other is just a switch with
 * settings - but on the grid they must look and behave the same. This wraps
 * either so the screen only ever deals in cards: a name, a switch, and the
 * settings behind a click.
 */
public final class ModEntry {
	public final String name;
	public final String description;
	/** The card's face; null falls back to the name's initial on a plate. */
	public final Icons.Icon icon;
	/** Set when the card is a HUD module; null for a standalone mod. */
	public final HudModule module;
	/** The switch of a standalone mod; null when {@link #module} is set. */
	private final Option.Bool toggle;
	/** What the settings view lists, in order. */
	public final List<Option> options;

	private ModEntry(String name, String description, Icons.Icon icon, HudModule module, Option.Bool toggle,
			List<Option> options) {
		this.name = name;
		this.description = description;
		this.icon = icon;
		this.module = module;
		this.toggle = toggle;
		this.options = options;
	}

	/** A HUD module: the shared display settings first, then its own. */
	public static ModEntry of(HudModule module) {
		List<Option> options = new ArrayList<>();
		options.add(module.scale);
		options.add(module.colour);
		options.add(module.shadow);
		options.add(module.background);
		options.add(module.backgroundColour);
		options.addAll(module.options);
		return new ModEntry(module.name, module.description, Icons.of(module.id), module, null, options);
	}

	/** A standalone mod: its switch, and whatever settings it brings. */
	public static ModEntry of(String name, String description, Option.Bool toggle, Option... options) {
		return new ModEntry(name, description, Icons.of(name), null, toggle, List.of(options));
	}

	public boolean enabled() {
		return module != null ? module.enabled : toggle.get();
	}

	public void toggle() {
		if (module != null) module.enabled = !module.enabled;
		else toggle.toggle();
		MangoConfig.save();
	}

	/** Whether the card has anything to open besides its switch. */
	public boolean hasSettings() {
		return !options.isEmpty();
	}

	public boolean matches(String query) {
		String q = query.toLowerCase(Locale.ROOT);
		return name.toLowerCase(Locale.ROOT).contains(q)
			|| description.toLowerCase(Locale.ROOT).contains(q);
	}
}
