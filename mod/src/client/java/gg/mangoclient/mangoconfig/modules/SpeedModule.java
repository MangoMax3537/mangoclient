package gg.mangoclient.mangoconfig.modules;

import gg.mangoclient.mangoconfig.Option;
import gg.mangoclient.mangoconfig.TextModule;
import net.minecraft.client.MinecraftClient;
import net.minecraft.client.font.TextRenderer;
import net.minecraft.client.gui.DrawContext;

import java.util.List;

/**
 * How fast the player is actually moving, in blocks per second.
 *
 * Measured from where the player was a moment ago rather than from velocity,
 * which on a client is zero as often as not, and smoothed a little so the
 * number is readable instead of flickering.
 */
public class SpeedModule extends TextModule {
	/** Long enough to be a real measurement, short enough to feel live. */
	private static final long SAMPLE_MS = 100;

	private final Option.Bool vertical = new Option.Bool("vertical", "Count falling and flying", false);
	private final Option.Bool label = new Option.Bool("label", "Show \"b/s\"", true);
	private final Option.Bool decimals = new Option.Bool("decimals", "Decimals", true);

	private double speed;
	private long sampledAt;
	private double lastX;
	private double lastY;
	private double lastZ;

	public SpeedModule() {
		super("speed", "Speed", "Blocks travelled per second.", false, 0.01f, 0.26f);
		options.add(vertical);
		options.add(label);
		options.add(decimals);
	}

	/** Called from render only, so measuring is not thrown off by layout calls. */
	private void sample(MinecraftClient mc) {
		if (mc.player == null) return;
		long now = System.currentTimeMillis();
		double x = mc.player.getX();
		double y = mc.player.getY();
		double z = mc.player.getZ();

		if (sampledAt == 0) {
			sampledAt = now;
			lastX = x;
			lastY = y;
			lastZ = z;
			return;
		}
		long dt = now - sampledAt;
		if (dt < SAMPLE_MS) return;

		double dx = x - lastX;
		double dy = vertical.value ? y - lastY : 0;
		double dz = z - lastZ;
		double instant = Math.sqrt(dx * dx + dy * dy + dz * dz) / (dt / 1000.0);

		speed = speed * 0.6 + instant * 0.4;
		sampledAt = now;
		lastX = x;
		lastY = y;
		lastZ = z;
	}

	@Override
	protected List<String> lines(MinecraftClient mc, boolean preview) {
		String value = decimals.value ? String.format("%.2f", speed) : String.valueOf(Math.round(speed));
		return List.of(label.value ? value + " b/s" : value);
	}

	@Override
	public int width(MinecraftClient mc, TextRenderer font) {
		// Sized for the widest it plausibly gets, so the plate does not breathe
		// in and out with every step.
		String widest = decimals.value ? "88.88" : "88";
		if (label.value) widest += " b/s";
		return font.getWidth(widest);
	}

	@Override
	public void render(DrawContext ctx, MinecraftClient mc, TextRenderer font, boolean preview) {
		sample(mc);
		super.render(ctx, mc, font, preview);
	}
}
