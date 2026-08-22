package gg.mangoclient.mangoconfig;

import java.util.ArrayList;
import java.util.List;

/**
 * One page of the settings screen.
 *
 * A category is either a grid of mod cards or a list of plain settings; the
 * screen picks the layout from which list is filled. A new page is a new
 * Category and nothing else.
 */
public class Category {
	public final String id;
	public final String name;
	public final String description;
	public final List<ModEntry> entries = new ArrayList<>();
	public final List<Option> options = new ArrayList<>();

	public Category(String id, String name, String description) {
		this.id = id;
		this.name = name;
		this.description = description;
	}

	public Category withEntries(List<ModEntry> list) {
		entries.addAll(list);
		return this;
	}

	/** Cards, not rows: this page is drawn as a grid. */
	public boolean grid() {
		return !entries.isEmpty();
	}

	public Category withOptions(List<Option> list) {
		options.addAll(list);
		return this;
	}

	public Category withOption(Option option) {
		options.add(option);
		return this;
	}
}
