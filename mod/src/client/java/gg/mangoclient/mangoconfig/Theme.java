package gg.mangoclient.mangoconfig;

/**
 * MangoClient's palette, so the in-game screens and the launcher look like the
 * same product. The values are the launcher's CSS custom properties from
 * src/renderer/css/app.css, as ARGB - keep the two in step.
 *
 * The greys are warm, pulled towards the brown in the mango's shadow, and the
 * accent is sampled from the logo itself. Green, amber and red are kept
 * strictly for status, exactly as the launcher uses them.
 */
public final class Theme {
	private Theme() {
	}

	public static final int BRAND = 0xFFE89A2C;
	public static final int BRAND_HOVER = 0xFFF6A93C;
	/** The bottom of a brand gradient. */
	public static final int BRAND_DEEP = 0xFFCD7F18;
	public static final int BRAND_FG = 0xFF2A1600;
	public static final int BRAND_QUIET = 0x38E89A2C;

	public static final int SURFACE_1 = 0xFF191614;
	public static final int SURFACE_2 = 0xFF201C18;
	public static final int SURFACE_3 = 0xFF2A2621;
	public static final int SURFACE_4 = 0xFF38332C;

	public static final int TEXT = 0xFFFFFFFF;
	public static final int TEXT_2 = 0xFFC9C1B5;
	public static final int TEXT_3 = 0xFFA89F92;

	/** Warm white at low opacity: a lit top edge, never a drawn line. */
	public static final int BTN_BORDER = 0x1AFFF0DC;
	public static final int HAIRLINE = 0x12FFF0DC;

	public static final int OK = 0xFF1BD96A;
	public static final int WARN = 0xFFF2A53C;
	public static final int DANGER = 0xFFFF4B4B;

	/** What sits behind a HUD element, dark enough to read over any world. */
	public static final int HUD_BG = 0x80191614;
	/** The dimmer behind a screen, so the world still shows through. */
	public static final int SCRIM = 0xC00D0B0A;

	/** The eight colours the picker offers, chosen to read on a bright world. */
	public static final int[] SWATCHES = {
		0xFFFFFFFF, 0xFFE89A2C, 0xFF1BD96A, 0xFF4A9EFF,
		0xFFFF4B4B, 0xFFC77DFF, 0xFFF2E14C, 0xFFA89F92,
	};

	/**
	 * The palette before it was warmed to match the launcher, old RGB to new.
	 *
	 * A colour is written into the config file the first time anything is
	 * saved, so without this every existing install would keep the cool greys
	 * on its HUD for ever and only new players would see the current client.
	 */
	private static final int[][] LEGACY = {
		{0xD98A3D, 0xE89A2C}, // brand
		{0xE79C52, 0xF6A93C}, // brand hover
		{0x1E1408, 0x2A1600}, // text on brand
		{0x16181C, 0x191614}, // surface 1, and the HUD plate behind an element
		{0x1D1F23, 0x201C18}, // surface 2
		{0x27292E, 0x2A2621}, // surface 3
		{0x34363C, 0x38332C}, // surface 4
		{0xB0BAC5, 0xC9C1B5}, // secondary text
		{0x96A2B0, 0xA89F92}, // tertiary text, and the grey swatch
	};

	/**
	 * Carry a stored colour over to the current palette, keeping whatever
	 * alpha the player set. Anything they picked themselves is left alone.
	 */
	public static int repaint(int argb) {
		int rgb = argb & 0x00FFFFFF;
		for (int[] pair : LEGACY) {
			if (pair[0] == rgb) return (argb & 0xFF000000) | pair[1];
		}
		return argb;
	}

	/** Blend `overlay` onto `base` by `alpha` (0..1), both opaque ARGB. */
	public static int mix(int base, int overlay, float alpha) {
		int r = (int) (((base >> 16) & 0xFF) * (1 - alpha) + ((overlay >> 16) & 0xFF) * alpha);
		int g = (int) (((base >> 8) & 0xFF) * (1 - alpha) + ((overlay >> 8) & 0xFF) * alpha);
		int b = (int) ((base & 0xFF) * (1 - alpha) + (overlay & 0xFF) * alpha);
		return 0xFF000000 | (r << 16) | (g << 8) | b;
	}

	public static int withAlpha(int color, int alpha) {
		return (color & 0x00FFFFFF) | ((alpha & 0xFF) << 24);
	}
}
