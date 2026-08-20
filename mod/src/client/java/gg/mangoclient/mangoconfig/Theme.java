package gg.mangoclient.mangoconfig;

/**
 * MangoClient's palette, so the in-game screens and the launcher look like the
 * same product. The values are the launcher's CSS custom properties, as ARGB.
 */
public final class Theme {
	private Theme() {
	}

	public static final int BRAND = 0xFFD98A3D;
	public static final int BRAND_HOVER = 0xFFE79C52;
	public static final int BRAND_FG = 0xFF1E1408;
	public static final int BRAND_QUIET = 0x40D98A3D;

	public static final int SURFACE_1 = 0xFF16181C;
	public static final int SURFACE_2 = 0xFF1D1F23;
	public static final int SURFACE_3 = 0xFF27292E;
	public static final int SURFACE_4 = 0xFF34363C;

	public static final int TEXT = 0xFFFFFFFF;
	public static final int TEXT_2 = 0xFFB0BAC5;
	public static final int TEXT_3 = 0xFF96A2B0;

	public static final int OK = 0xFF1BD96A;
	public static final int WARN = 0xFFF2A53C;
	public static final int DANGER = 0xFFFF4B4B;

	/** What sits behind a HUD element, dark enough to read over any world. */
	public static final int HUD_BG = 0x8016181C;
	/** The dimmer behind a screen, so the world still shows through. */
	public static final int SCRIM = 0xC00A0B0D;

	/** The eight colours the picker offers, chosen to read on a bright world. */
	public static final int[] SWATCHES = {
		0xFFFFFFFF, 0xFFD98A3D, 0xFF1BD96A, 0xFF4A9EFF,
		0xFFFF4B4B, 0xFFC77DFF, 0xFFF2E14C, 0xFF96A2B0,
	};

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
