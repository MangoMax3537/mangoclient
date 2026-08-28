package gg.mangoclient.mangoconfig;

import com.google.gson.JsonObject;
import net.minecraft.client.util.InputUtil;

import java.util.function.BooleanSupplier;
import java.util.function.Consumer;
import java.util.function.IntConsumer;
import java.util.function.IntSupplier;
import java.util.function.Supplier;

/**
 * One customisable setting.
 *
 * A setting either owns its value or is *bound* to somewhere else - Minecraft's
 * own options, say. A bound setting reads and writes through its accessors and
 * is never written to MangoConfig's own file, because whoever owns it already
 * persists it.
 *
 * The settings screen walks a list of these and draws a row per kind, so a new
 * setting anywhere needs no screen changes at all.
 */
public abstract class Option {
	public final String key;
	public final String label;
	/** Shown under the label when the name alone does not explain it. */
	public String hint = "";
	/** True when the value lives somewhere else and must not be saved here. */
	protected boolean bound;

	protected Option(String key, String label) {
		this.key = key;
		this.label = label;
	}

	public Option hint(String text) {
		this.hint = text;
		return this;
	}

	public boolean isBound() {
		return bound;
	}

	public abstract void save(JsonObject json);

	public abstract void load(JsonObject json);

	/** A switch. */
	public static class Bool extends Option {
		public boolean value;
		private BooleanSupplier getter;
		private Consumer<Boolean> setter;

		public Bool(String key, String label, boolean value) {
			super(key, label);
			this.value = value;
		}

		public Bool bind(BooleanSupplier getter, Consumer<Boolean> setter) {
			this.getter = getter;
			this.setter = setter;
			this.bound = true;
			return this;
		}

		public boolean get() {
			return getter != null ? getter.getAsBoolean() : value;
		}

		public void set(boolean v) {
			if (setter != null) setter.accept(v);
			else value = v;
		}

		public void toggle() {
			set(!get());
		}

		@Override
		public void save(JsonObject json) {
			if (!bound) json.addProperty(key, value);
		}

		@Override
		public void load(JsonObject json) {
			if (!bound && json.has(key)) value = json.get(key).getAsBoolean();
		}
	}

	/** A slider over whole numbers. */
	public static class Range extends Option {
		public int value;
		public final int min;
		public final int max;
		public final String suffix;
		private IntSupplier getter;
		private IntConsumer setter;

		public Range(String key, String label, int value, int min, int max, String suffix) {
			super(key, label);
			this.value = value;
			this.min = min;
			this.max = max;
			this.suffix = suffix;
		}

		public Range bind(IntSupplier getter, IntConsumer setter) {
			this.getter = getter;
			this.setter = setter;
			this.bound = true;
			return this;
		}

		public int get() {
			int v = getter != null ? getter.getAsInt() : value;
			return Math.max(min, Math.min(max, v));
		}

		public void set(int v) {
			int clamped = Math.max(min, Math.min(max, v));
			if (setter != null) setter.accept(clamped);
			else value = clamped;
		}

		/** Set from a 0..1 slider position. */
		public void setFraction(double f) {
			set(min + (int) Math.round(Math.max(0, Math.min(1, f)) * (max - min)));
		}

		public float fraction() {
			return max == min ? 0f : (get() - min) / (float) (max - min);
		}

		public String display() {
			return get() + suffix;
		}

		@Override
		public void save(JsonObject json) {
			if (!bound) json.addProperty(key, value);
		}

		@Override
		public void load(JsonObject json) {
			if (!bound && json.has(key)) value = Math.max(min, Math.min(max, json.get(key).getAsInt()));
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
			if (json.has(key)) value = Theme.repaint(json.get(key).getAsInt());
		}
	}

	/** One of a fixed set, cycled by clicking. */
	public static class Choice extends Option {
		public int index;
		public final String[] values;
		private IntSupplier getter;
		private IntConsumer setter;

		public Choice(String key, String label, int index, String... values) {
			super(key, label);
			this.values = values;
			this.index = Math.max(0, Math.min(values.length - 1, index));
		}

		public Choice bind(IntSupplier getter, IntConsumer setter) {
			this.getter = getter;
			this.setter = setter;
			this.bound = true;
			return this;
		}

		public int get() {
			int v = getter != null ? getter.getAsInt() : index;
			return Math.max(0, Math.min(values.length - 1, v));
		}

		public String value() {
			return values[get()];
		}

		public boolean is(String name) {
			return value().equals(name);
		}

		public void next() {
			int v = (get() + 1) % values.length;
			if (setter != null) setter.accept(v);
			else index = v;
		}

		@Override
		public void save(JsonObject json) {
			if (!bound) json.addProperty(key, values[index]);
		}

		@Override
		public void load(JsonObject json) {
			if (bound || !json.has(key)) return;
			String want = json.get(key).getAsString();
			for (int i = 0; i < values.length; i++) {
				if (values[i].equals(want)) {
					index = i;
					return;
				}
			}
		}
	}

	/**
	 * A key to hold or press.
	 *
	 * Clicking the row starts listening and the next key pressed is taken, the
	 * way every game does it - a list of key names to cycle through would be
	 * unusable at this length.
	 */
	public static class Key extends Option {
		public int value;
		/** True while the row is waiting for a key press. */
		public boolean listening;

		public Key(String key, String label, int value) {
			super(key, label);
			this.value = value;
		}

		public String display() {
			if (listening) return "press a key";
			return InputUtil.Type.KEYSYM.createFromCode(value).getLocalizedText().getString();
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

	/** A row that only reads: something worth showing with nothing to change. */
	public static class Readout extends Option {
		public final Supplier<String> value;

		public Readout(String label, Supplier<String> value) {
			super("readout", label);
			this.value = value;
			this.bound = true;
		}

		@Override
		public void save(JsonObject json) {
		}

		@Override
		public void load(JsonObject json) {
		}
	}
}
