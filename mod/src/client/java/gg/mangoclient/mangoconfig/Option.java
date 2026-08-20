package gg.mangoclient.mangoconfig;

import com.google.gson.JsonObject;

/**
 * One customisable setting on a module.
 *
 * Every module is a bag of these, which is what lets the settings screen draw
 * any module without knowing what it is: it walks the list and renders a row
 * per kind. Adding a setting to a module needs no screen changes at all.
 */
public abstract class Option {
	public final String key;
	public final String label;
	/** Shown under the label when the name alone does not explain it. */
	public String hint = "";

	protected Option(String key, String label) {
		this.key = key;
		this.label = label;
	}

	public Option hint(String text) {
		this.hint = text;
		return this;
	}

	public abstract void save(JsonObject json);

	public abstract void load(JsonObject json);

	/** A checkbox. */
	public static class Bool extends Option {
		public boolean value;

		public Bool(String key, String label, boolean value) {
			super(key, label);
			this.value = value;
		}

		public void toggle() {
			value = !value;
		}

		@Override
		public void save(JsonObject json) {
			json.addProperty(key, value);
		}

		@Override
		public void load(JsonObject json) {
			if (json.has(key)) value = json.get(key).getAsBoolean();
		}
	}

	/** A slider over whole numbers. */
	public static class Range extends Option {
		public int value;
		public final int min;
		public final int max;
		public final String suffix;

		public Range(String key, String label, int value, int min, int max, String suffix) {
			super(key, label);
			this.value = value;
			this.min = min;
			this.max = max;
			this.suffix = suffix;
		}

		/** Set from a 0..1 slider position. */
		public void setFraction(double f) {
			value = min + (int) Math.round(Math.max(0, Math.min(1, f)) * (max - min));
		}

		public float fraction() {
			return max == min ? 0f : (value - min) / (float) (max - min);
		}

		public String display() {
			return value + suffix;
		}

		@Override
		public void save(JsonObject json) {
			json.addProperty(key, value);
		}

		@Override
		public void load(JsonObject json) {
			if (json.has(key)) value = Math.max(min, Math.min(max, json.get(key).getAsInt()));
		}
	}

	/** A colour, picked from the palette. Stored as ARGB so alpha survives. */
	public static class Colour extends Option {
		public int value;

		public Colour(String key, String label, int value) {
			super(key, label);
			this.value = value;
		}

		@Override
		public void save(JsonObject json) {
			json.addProperty(key, value);
		}

		@Override
		public void load(JsonObject json) {
			if (json.has(key)) value = json.get(key).getAsInt();
		}
	}

	/** One of a fixed set, cycled by clicking. */
	public static class Choice extends Option {
		public int index;
		public final String[] values;

		public Choice(String key, String label, int index, String... values) {
			super(key, label);
			this.values = values;
			this.index = Math.max(0, Math.min(values.length - 1, index));
		}

		public String value() {
			return values[index];
		}

		public boolean is(String name) {
			return value().equals(name);
		}

		public void next() {
			index = (index + 1) % values.length;
		}

		@Override
		public void save(JsonObject json) {
			json.addProperty(key, value());
		}

		@Override
		public void load(JsonObject json) {
			if (!json.has(key)) return;
			String want = json.get(key).getAsString();
			for (int i = 0; i < values.length; i++) {
				if (values[i].equals(want)) {
					index = i;
					return;
				}
			}
		}
	}
}
